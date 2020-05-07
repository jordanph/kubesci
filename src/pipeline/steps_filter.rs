use vec1::Vec1;
use crate::pipeline::Step;

pub fn filter<'a>(steps: &'a[Step], github_branch_name: &String) -> Option<Vec1<&'a Step>> {
  let maybe_steps = steps
      .iter()
      .filter(|step| skip_step(step, github_branch_name))
      .collect::<Vec<_>>();

  return Vec1::try_from_vec(maybe_steps).ok();
}

fn skip_step(step: &Step, github_branch_name: &String) -> bool {
  return step.branch.is_none() || step.branch == Some(github_branch_name.to_string()) || not_branch(step.branch.as_ref(), github_branch_name);
}

fn not_branch(branch: Option<&String>, github_branch_name: &String) -> bool {
  return branch.map(|branch| branch.chars().next() == Some('!') && branch[1..] != github_branch_name.to_string()).unwrap_or(false);
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn should_return_none_if_no_steps_to_run() {
      let empty_steps = &Vec::new();
      let maybe_steps = filter(empty_steps, &"some_branch".to_string());

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
          mount_secret: None
      };

      let step_that_does_not_match_branch = Step {
          name: "step_that_does_not_match_branch".to_string(),
          image: "some_image".to_string(),
          commands: None,
          args: None,
          branch: Some("some_other_branch".to_string()),
          env: None,
          mount_secret: None
      };

      let steps = vec![step_that_does_not_match_branch, step_that_matches_branch];

      let filtered_steps = filter(&steps, &branch.to_string()).unwrap();

      let filter_step_names: Vec<String> = filtered_steps.into_iter().map(|step| step.name.clone()).collect();

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
          mount_secret: None
      };

      let step_that_does_not_match_branch = Step {
          name: "step_that_does_not_match_branch".to_string(),
          image: "some_image".to_string(),
          commands: None,
          args: None,
          branch: Some("some_other_branch".to_string()),
          env: None,
          mount_secret: None
      };

      let steps = vec![step_that_does_not_match_branch, step_that_matches_branch];

      let filtered_steps = filter(&steps, &branch.to_string()).unwrap();

      let filter_step_names: Vec<String> = filtered_steps.into_iter().map(|step| step.name.clone()).collect();

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
          mount_secret: None
      };

      let step_with_exclamation_branch_that_does_not_match_branch = Step {
          name: "step_with_exclamation_branch_that_does_not_match_branch".to_string(),
          image: "some_image".to_string(),
          commands: None,
          args: None,
          branch: Some("!some_other_branch".to_string()),
          env: None,
          mount_secret: None
      };

      let steps = vec![step_with_exclamation_branch_that_matches_branch, step_with_exclamation_branch_that_does_not_match_branch];

      let filtered_steps = filter(&steps, &branch.to_string()).unwrap();

      let filter_step_names: Vec<String> = filtered_steps.into_iter().map(|step| step.name.clone()).collect();

      assert!(!filter_step_names.contains(&"step_with_exclamation_branch_that_matches_branch".to_string()));
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
          mount_secret: None
      };

      let step_with_exclamation_branch_that_does_not_match_branch = Step {
          name: "step_with_exclamation_branch_that_does_not_match_branch".to_string(),
          image: "some_image".to_string(),
          commands: None,
          args: None,
          branch: Some("!some_other_branch".to_string()),
          env: None,
          mount_secret: None
      };

      let steps = vec![step_with_exclamation_branch_that_matches_branch, step_with_exclamation_branch_that_does_not_match_branch];

      let filtered_steps = filter(&steps, &branch.to_string()).unwrap();

      let filter_step_names: Vec<String> = filtered_steps.into_iter().map(|step| step.name.clone()).collect();

      assert!(filter_step_names.contains(&"step_with_exclamation_branch_that_does_not_match_branch".to_string()));
  }
}
