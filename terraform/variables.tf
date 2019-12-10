variable "project_name" {
  type        = string
  description = "GCP project name to deploy to."
}

variable "region" {
  type    = string
  default = "us-east1"
}

variable "zone" {
  type    = string
  default = "us-east1-b"
}
