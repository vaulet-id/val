output "url" {
  value       = google_cloud_run_v2_service.runner.uri
  description = "Set VITE_RUNNER to this when building the playground"
}

output "repository" {
  value = "${var.region}-docker.pkg.dev/${var.project}/${google_artifact_registry_repository.runner.repository_id}"
}
