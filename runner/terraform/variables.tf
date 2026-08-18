variable "project" {
  type        = string
  description = "GCP project id"
}

variable "region" {
  type    = string
  default = "asia-southeast1"
}

variable "image" {
  type        = string
  description = "Full image ref, e.g. asia-southeast1-docker.pkg.dev/PROJECT/val/runner:TAG"
}

variable "cpu" {
  type    = string
  default = "2"
}

variable "memory" {
  type    = string
  default = "2Gi"
}

# Cold start is a Rust or Go compile, so the first request after a scale to
# zero is the slow one. Keep one warm where that matters.
variable "min_instances" {
  type    = number
  default = 0
}

variable "max_instances" {
  type    = number
  default = 10
}

variable "public" {
  type        = bool
  default     = true
  description = "Whether anyone may invoke it. The playground needs this; a private deployment does not."
}
