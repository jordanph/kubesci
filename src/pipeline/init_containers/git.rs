use serde_json::json;
use vec1::Vec1;

pub fn generate_git_init_container(repo_name: &str, github_head_sha: &str, volume_mount_names: &Vec1<&str>) -> serde_json::Value {
  let clone_url = format!("https://github.com/{}", repo_name);

  let container_volumes = volume_mount_names
    .into_iter()
    .map(|volume_name| format!("/{}", volume_name))
    .collect::<Vec<String>>()
    .join(";");

  let volume_mounts = volume_mount_names
    .into_iter()
    .map(|volume_name| json!({
      "name": volume_name,
      "value": format!("/{}", volume_name)
    })).collect::<Vec<serde_json::value::Value>>();

  json!({
    "name": "kubes-cd-git-checkout",
    "image": "jordanph/kubes-cd-git-checkout:latest",
    "workingDir": "/app",
    "volumeMounts": volume_mounts,
    "env": [
      {
        "name": "REPO_URL",
        "value": clone_url
      },
      {
        "name": "COMMIT_SHA",
        "value": github_head_sha
      },
      {
        "name": "CONTAINER_VOLUMES",
        "value": container_volumes
      }
    ]
  })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn should_correctly_set_repo_url_as_env_variable() {
      let repo_name = "test-repo/test";
      let git_init_container = generate_git_init_container(repo_name, "12nasf0", &vec1!(""));
      
      let repo_url_env = git_init_container.pointer("/env/0").unwrap();

      let expected_value = json!({
        "name": "REPO_URL",
        "value": format!("https://github.com/{}", repo_name)
      });

      assert_eq!(repo_url_env, &expected_value)
    }

    #[test]
    fn should_correctly_set_commit_sha_as_env_variable() {
      let commit_sha = "f1fsaf13";
      let git_init_container = generate_git_init_container("whatever", commit_sha, &vec1!(""));
      
      let commit_sha_env = git_init_container.pointer("/env/1").unwrap();

      let expected_value = json!({
        "name": "COMMIT_SHA",
        "value": commit_sha
      });

      assert_eq!(commit_sha_env, &expected_value)
    }

    #[test]
    fn should_separate_container_volumes_by_semicolon() {
      let container_volume_names = vec1!("test", "google", "house");
      let git_init_container = generate_git_init_container("whatever", "something", &container_volume_names);
      
      let container_volumes_env = git_init_container.pointer("/env/2").unwrap();

      let expected_value = json!({
        "name": "CONTAINER_VOLUMES",
        "value": "/test;/google;/house"
      });

      assert_eq!(container_volumes_env, &expected_value)
    }

    #[test]
    fn should_correct_construct_volume_mounts() {
      let container_volume_names = vec1!("test", "google", "house");
      let git_init_container = generate_git_init_container("whatever", "something", &container_volume_names);
      
      let container_volumes_env = git_init_container.pointer("/volumeMounts").unwrap();

      let expected_value = json!([{
          "name": "test",
          "value": "/test"
        },
        {
          "name": "google",
          "value": "/google"
        },
        {
          "name": "house",
          "value": "/house"
        }]);

      assert_eq!(container_volumes_env, &expected_value)
    }
}

