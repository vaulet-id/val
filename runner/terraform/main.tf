# The runner, on Cloud Run.
#
# A handler is somebody else's code, so the isolation matters more than the
# hosting does. Cloud Run runs each revision in a gVisor sandbox with no
# ambient credentials, which is the boundary this service relies on; the runner
# itself adds a clean environment, a fresh directory and a wall-clock limit per
# request.

terraform {
  required_version = ">= 1.6"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }
}

provider "google" {
  project = var.project
  region  = var.region
}

resource "google_artifact_registry_repository" "runner" {
  location      = var.region
  repository_id = "val"
  format        = "DOCKER"
  description   = "Images for the VAL runner"
}

# Its own identity, with nothing granted to it. The runner reads no bucket and
# calls no API: it is handed a record and answers with a decision.
resource "google_service_account" "runner" {
  account_id   = "val-runner"
  display_name = "VAL runner"
}

resource "google_cloud_run_v2_service" "runner" {
  name     = "val-runner"
  location = var.region

  template {
    service_account = google_service_account.runner.email

    # Compiling Go or Rust is what the CPU is for, and it is bursty. One
    # request at a time per instance keeps one handler's compile from starving
    # another's.
    max_instance_request_concurrency = 1
    timeout                          = "60s"

    scaling {
      min_instance_count = var.min_instances
      max_instance_count = var.max_instances
    }

    containers {
      image = var.image

      resources {
        limits = {
          cpu    = var.cpu
          memory = var.memory
        }
      }

      env {
        name  = "PORT"
        value = "8787"
      }

      ports {
        container_port = 8787
      }
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

# Public, because the playground is a static site with no back end of its own
# and the service holds nothing: every request carries its own record, and the
# answer is a decision the caller could have computed with the same inputs.
resource "google_cloud_run_v2_service_iam_member" "public" {
  count    = var.public ? 1 : 0
  project  = google_cloud_run_v2_service.runner.project
  location = google_cloud_run_v2_service.runner.location
  name     = google_cloud_run_v2_service.runner.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}
