//! Provides testing infrastructure for rover-http consumers

use rover_tower::mock_service;

use crate::{HttpRequest, HttpResponse, HttpServiceError};

// Provides a MockHttpService
mock_service!(Http, HttpRequest, HttpResponse, HttpServiceError);
