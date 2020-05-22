use crate::pipeline::{Block, Step, StepType};
use either::{Either, Either::Left, Either::Right};
use vec1::Vec1;

pub fn filter<'a>(
    steps: &'a [StepType],
    github_branch_name: &str,
    step_section: usize,
) -> Option<Either<&'a Block, Vec1<&'a Step>>> {
    let maybe_steps = steps
        .iter()
        .filter(|step| skip_step_or_block(step, github_branch_name))
        .collect::<Vec<_>>();

    match Vec1::try_from_vec(maybe_steps).ok() {
        Some(steps) => {
            let split_steps = split_into_blocks_and_steps(steps);

            split_steps
                .get(step_section)
                .map(|either| either.to_owned())
        }
        None => None,
    }
}

// There be dragons...
fn split_into_blocks_and_steps<'a>(
    steps_or_blocks: Vec1<&'a StepType>,
) -> Vec<Either<&'a Block, Vec1<&'a Step>>> {
    let mut previous_step_was_wait = false;

    steps_or_blocks.iter().fold(
        Vec::new(),
        |mut acc: Vec<Either<&'a Block, Vec1<&'a Step>>>, block_or_step| match block_or_step {
            StepType::Block(block) => {
                acc.push(Left(&block));
                previous_step_was_wait = false;
                acc
            }
            StepType::Step(step) => {
                let lastest_value = acc.pop();

                match lastest_value {
                    Some(Right(steps)) if !previous_step_was_wait => {
                        let mut non_empty_vec = vec1![step];
                        non_empty_vec.extend(steps.to_owned());

                        acc.push(Right(non_empty_vec));
                    }
                    Some(previous) => {
                        acc.push(previous);
                        acc.push(Right(vec1![&step]));
                    }
                    None => acc.push(Right(vec1![&step])),
                }

                previous_step_was_wait = false;
                acc
            }
            StepType::Wait => {
                previous_step_was_wait = true;
                acc
            }
        },
    )
}

fn skip_step_or_block(step: &StepType, github_branch_name: &str) -> bool {
    let branch = match step {
        StepType::Block(block) => block.branch.clone(),
        StepType::Step(step) => step.branch.clone(),
        StepType::Wait => None,
    };

    branch.is_none()
        || branch == Some(github_branch_name.to_string())
        || not_branch(branch.as_ref(), github_branch_name)
}

fn not_branch(branch: Option<&String>, github_branch_name: &str) -> bool {
    branch
        .map(|branch| branch.starts_with('!') && branch[1..] != *github_branch_name)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn should_return_none_if_no_steps_to_run() {
        let empty_steps = &Vec::new();
        let maybe_steps = filter(empty_steps, &"some_branch".to_string(), 0);

        assert!(maybe_steps.is_none());
    }

    #[test]
    fn should_filter_steps_with_defined_branch_that_does_not_match_current() {
        let branch = "master";

        let step_that_matches_branch = Step {
            name: "step_that_matches_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some(branch.to_string()),
            env: None,
            mount_secret: None,
        };

        let step_that_does_not_match_branch = Step {
            name: "step_that_does_not_match_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some("some_other_branch".to_string()),
            env: None,
            mount_secret: None,
        };

        let steps = vec![
            StepType::Step(step_that_does_not_match_branch),
            StepType::Step(step_that_matches_branch),
        ];

        let filtered_steps = filter(&steps, branch, 0).unwrap().right().unwrap();

        let filter_step_names: Vec<String> = filtered_steps
            .iter()
            .map(|step| step.name.clone())
            .collect();

        assert!(!filter_step_names.contains(&"step_that_does_not_match_branch".to_string()));
    }

    #[test]
    fn should_not_filter_steps_with_defined_branch_that_does_not_match_current() {
        let branch = "master";

        let step_that_matches_branch = Step {
            name: "step_that_matches_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some(branch.to_string()),
            env: None,
            mount_secret: None,
        };

        let step_that_does_not_match_branch = Step {
            name: "step_that_does_not_match_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some("some_other_branch".to_string()),
            env: None,
            mount_secret: None,
        };

        let steps = vec![
            StepType::Step(step_that_does_not_match_branch),
            StepType::Step(step_that_matches_branch),
        ];

        let filtered_steps = filter(&steps, branch, 0).unwrap().right().unwrap();

        let filter_step_names: Vec<String> = filtered_steps
            .iter()
            .map(|step| step.name.clone())
            .collect();

        assert!(filter_step_names.contains(&"step_that_matches_branch".to_string()));
    }

    #[test]
    fn should_filter_steps_with_defined_exclamation_branch_that_matches_branch() {
        let branch = "master";

        let step_with_exclamation_branch_that_matches_branch = Step {
            name: "step_with_exclamation_branch_that_matches_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some(format!("!{}", branch)),
            env: None,
            mount_secret: None,
        };

        let step_with_exclamation_branch_that_does_not_match_branch = Step {
            name: "step_with_exclamation_branch_that_does_not_match_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some("!some_other_branch".to_string()),
            env: None,
            mount_secret: None,
        };

        let steps = vec![
            StepType::Step(step_with_exclamation_branch_that_matches_branch),
            StepType::Step(step_with_exclamation_branch_that_does_not_match_branch),
        ];

        let filtered_steps = filter(&steps, branch, 0).unwrap().right().unwrap();

        let filter_step_names: Vec<String> = filtered_steps
            .iter()
            .map(|step| step.name.clone())
            .collect();

        assert!(!filter_step_names
            .contains(&"step_with_exclamation_branch_that_matches_branch".to_string()));
    }

    #[test]
    fn should_not_filter_steps_with_defined_exclamation_branch_that_do_not_match_branch() {
        let branch = "master";

        let step_with_exclamation_branch_that_matches_branch = Step {
            name: "step_with_exclamation_branch_that_matches_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some(format!("!{}", branch)),
            env: None,
            mount_secret: None,
        };

        let step_with_exclamation_branch_that_does_not_match_branch = Step {
            name: "step_with_exclamation_branch_that_does_not_match_branch".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: Some("!some_other_branch".to_string()),
            env: None,
            mount_secret: None,
        };

        let steps = vec![
            StepType::Step(step_with_exclamation_branch_that_matches_branch),
            StepType::Step(step_with_exclamation_branch_that_does_not_match_branch),
        ];

        let filtered_steps = filter(&steps, branch, 0).unwrap().right().unwrap();

        let filter_step_names: Vec<String> = filtered_steps
            .iter()
            .map(|step| step.name.clone())
            .collect();

        assert!(filter_step_names
            .contains(&"step_with_exclamation_branch_that_does_not_match_branch".to_string()));
    }

    #[test]
    fn should_return_steps_if_before_block() {
        let step = Step {
            name: "step".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: None,
            env: None,
            mount_secret: None,
        };

        let block = Block {
            name: "block".to_string(),
            branch: None,
        };

        let steps = vec![StepType::Step(step), StepType::Block(block)];

        let filtered_steps = filter(&steps, "some_branch", 0).unwrap();

        assert!(filtered_steps.is_right());
    }

    #[test]
    fn should_return_block_if_before_steps() {
        let step = Step {
            name: "step".to_string(),
            image: "some_image".to_string(),
            commands: None,
            args: None,
            branch: None,
            env: None,
            mount_secret: None,
        };

        let block = Block {
            name: "block".to_string(),
            branch: None,
        };

        let steps = vec![StepType::Block(block), StepType::Step(step)];

        let filtered_steps = filter(&steps, "some_branch", 0).unwrap();

        assert!(filtered_steps.is_left());
    }
}
