extern crate command;

use crate::command::init::graph_id_operations::{
  validate_and_check_availability, generate_graph_id
};
use crate::command::init::GraphIdOpt;
use crate::RoverError;

use clap::Parser;
use std::sync::Arc;
use tokio::test;

#[cfg(test)]
mod graph_id_operations_mocks {
  use super::*;
  use std::cell::RefCell;
  use rover_client::blocking::StudioClient;
  
  thread_local! {
      static VALIDATE_MOCK: RefCell<Option<Box<dyn Fn(&str) -> Result<(), RoverError>>>> = RefCell::new(None);
  }
  
  pub fn mock_validate(mock_fn: impl Fn(&str) -> Result<(), RoverError> + 'static) {
      VALIDATE_MOCK.with(|cell| {
          *cell.borrow_mut() = Some(Box::new(mock_fn));
      });
  }
  
  pub fn reset_validate_mock() {
      VALIDATE_MOCK.with(|cell| {
          *cell.borrow_mut() = None;
      });
  }
  
  pub async fn mock_validate_impl(graph_id: &str, _client: &StudioClient) -> Result<(), RoverError> {
      VALIDATE_MOCK.with(|cell| {
          if let Some(mock) = &*cell.borrow() {
              return mock(graph_id);
          }
          Ok(())
      })
  }
}

#[cfg(test)]
pub use graph_id_operations_mocks::{
  mock_validate,
  reset_validate_mock,
  mock_validate_impl as validate_and_check_availability
};

// Mock for generate_graph_id
#[cfg(test)]
pub fn generate_graph_id(project_name: &str) -> String {
  format!("{}-generated", project_name)
}

// Mock for dialoguer Input
#[cfg(test)]
mod dialoguer_mock {
  use std::cell::RefCell;
  
  thread_local! {
      static INPUT_RESPONSES: RefCell<Vec<String>> = RefCell::new(Vec::new());
  }
  
  pub fn mock_user_input(responses: Vec<String>) {
      INPUT_RESPONSES.with(|cell| {
          *cell.borrow_mut() = responses;
      });
  }
  
  pub fn get_next_input() -> Option<String> {
      INPUT_RESPONSES.with(|cell| {
          let mut responses = cell.borrow_mut();
          if responses.is_empty() {
              None
          } else {
              Some(responses.remove(0))
          }
      })
  }
}

// Mock implementation for dialoguer::Input
#[cfg(test)]
pub mod dialoguer {
  use super::dialoguer_mock::get_next_input;
  
  pub struct Input<T> {
      _prompt: String,
      _default: Option<T>,
      _allow_empty: bool,
  }
  
  impl<T: std::str::FromStr + Clone> Input<T> {
      pub fn new() -> Self {
          Self {
              _prompt: String::new(),
              _default: None,
              _allow_empty: true,
          }
      }
      
      pub fn with_prompt(mut self, prompt: &str) -> Self {
          self._prompt = prompt.to_string();
          self
      }
      
      pub fn default(mut self, default: T) -> Self {
          self._default = Some(default);
          self
      }
      
      pub fn allow_empty(mut self, allow: bool) -> Self {
          self._allow_empty = allow;
          self
      }
      
      pub fn interact(self) -> Result<T, std::io::Error> {
          if let Some(input) = get_next_input() {
              if input.is_empty() && !self._allow_empty {
                  return Err(std::io::Error::new(
                      std::io::ErrorKind::InvalidInput,
                      "Empty input not allowed",
                  ));
              }
              
              match input.parse::<T>() {
                  Ok(parsed) => Ok(parsed),
                  Err(_) => {
                      if let Some(default) = self._default {
                          Ok(default)
                      } else {
                          Err(std::io::Error::new(
                              std::io::ErrorKind::InvalidInput,
                              "Failed to parse input",
                          ))
                      }
                  }
              }
          } else if let Some(default) = self._default {
              Ok(default)
          } else {
              Err(std::io::Error::new(
                  std::io::ErrorKind::NotFound,
                  "No input provided and no default available",
              ))
          }
      }
  }
}

#[derive(Clone)]
struct TestClient;

#[cfg(test)]
mod tests {
  use super::*;
  use clap::Parser;
  use std::sync::Arc;

  #[test]
fn test_graph_id_opt_default() {
  let opt = GraphIdOpt::default();
  assert!(opt.graph_id.is_none());
}

#[test]
fn test_get_initial_graph_id_with_provided_id() {
  let opt = GraphIdOpt {
      graph_id: Some("my-custom-id".to_string()),
  };
  
  let result = opt.get_initial_graph_id("test-project");
  assert_eq!(result, "my-custom-id");
}

#[test]
fn test_get_initial_graph_id_with_generated_id() {
  let opt = GraphIdOpt::default();
  
  let result = opt.get_initial_graph_id("test-project");
  assert_eq!(result, "test-project-generated");
}

#[test]
fn test_parser() {
  let opt = GraphIdOpt::parse_from(&["program", "--graph-id", "custom-graph"]);
  assert_eq!(opt.graph_id, Some("custom-graph".to_string()));
}

#[tokio::test]
async fn test_prompt_for_input() {
  // Mock user input
  dialoguer_mock::mock_user_input(vec!["custom-input".to_string()]);
  
  let opt = GraphIdOpt::default();
  let result = opt.prompt_for_input("default-id");
  
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), "custom-input");
}

#[tokio::test]
async fn test_prompt_for_input_with_empty_input() {
  // Empty input should return the default since allow_empty is false
  dialoguer_mock::mock_user_input(vec!["".to_string()]);
  
  let opt = GraphIdOpt::default();
  let result = opt.prompt_for_input("default-id");
  
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), "default-id");
}

#[tokio::test]
async fn test_handle_validation_error_with_retries_left() {
  let opt = GraphIdOpt::default();
  let error = RoverError::new("Test error");
  
  // With retries left (attempt 1 of 3), should return Ok
  let result = opt.handle_validation_error(error, 1, 3);
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_handle_validation_error_on_last_retry() {
  let opt = GraphIdOpt::default();
  let error = RoverError::new("Test error");
  
  // On last retry (attempt 3 of 3), should return Err
  let result = opt.handle_validation_error(error, 3, 3);
  assert!(result.is_err());
}

#[tokio::test]
async fn test_get_or_prompt_graph_id_with_provided_id() {
  // Set up mock 
  mock_validate(|id| {
      assert_eq!(id, "provided-id");
      Ok(())
  });
  
  let opt = GraphIdOpt {
      graph_id: Some("provided-id".to_string()),
  };
  
  let client = TestClient;
  let result = opt.get_or_prompt_graph_id(&client, "test-project").await;
  
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), "provided-id");
  
  // Clean up mock
  reset_validate_mock();
}

#[tokio::test]
async fn test_get_or_prompt_graph_id_with_user_accepting_default() {
  // Mock user accepting the default (empty input)
  dialoguer_mock::mock_user_input(vec!["".to_string()]);

  mock_validate(|id| {
      assert_eq!(id, "test-project-generated");
      Ok(())
  });
  
  let opt = GraphIdOpt::default();
  let client = TestClient;
  
  let result = opt.get_or_prompt_graph_id(&client, "test-project").await;
  
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), "test-project-generated");

  reset_validate_mock();
}

#[tokio::test]
async fn test_get_or_prompt_graph_id_with_user_providing_custom_id() {
  dialoguer_mock::mock_user_input(vec!["custom-user-id".to_string()]);

  mock_validate(|id| {
      assert_eq!(id, "custom-user-id");
      Ok(())
  });
  
  let opt = GraphIdOpt::default();
  let client = TestClient;
  
  let result = opt.get_or_prompt_graph_id(&client, "test-project").await;
  
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), "custom-user-id");
  
  reset_validate_mock();
}

#[tokio::test]
async fn test_get_or_prompt_graph_id_retry_on_error() {
  // Mock user making 2 attempts - first fails validation, second succeeds
  dialoguer_mock::mock_user_input(vec![
      "invalid-id".to_string(),
      "valid-id".to_string(),
  ]);
  
  // Set up mock for validation - fails first, succeeds second
  let validation_calls = Arc::new(std::sync::Mutex::new(0));
  
  mock_validate(move |id| {
      let mut calls = validation_calls.lock().unwrap();
      *calls += 1;
      
      match *calls {
          1 => {
              assert_eq!(id, "invalid-id");
              Err(RoverError::new("Invalid graph ID"))
          },
          2 => {
              assert_eq!(id, "valid-id");
              Ok(())
          },
          _ => panic!("Unexpected number of validation calls"),
      }
  });
  
  let opt = GraphIdOpt::default();
  let client = TestClient;
  
  let result = opt.get_or_prompt_graph_id(&client, "test-project").await;
  
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), "valid-id");

  reset_validate_mock();
}

#[tokio::test]
async fn test_get_or_prompt_graph_id_max_retries() {
  // Mock user failing 3 attempts (MAX_RETRIES)
  dialoguer_mock::mock_user_input(vec![
      "invalid-id-1".to_string(),
      "invalid-id-2".to_string(),
      "invalid-id-3".to_string(),
  ]);
  
  // Set up mock for validation - all attempts fail
  mock_validate(|_| {
      Err(RoverError::new("Invalid graph ID"))
  });
  
  let opt = GraphIdOpt::default();
  let client = TestClient;
  
  let result = opt.get_or_prompt_graph_id(&client, "test-project").await;
  
  // After MAX_RETRIES failures, it should propagate the error
  assert!(result.is_err());
  reset_validate_mock();
}
}