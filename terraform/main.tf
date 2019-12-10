provider "google" {
  version     = "~> 3.3"
  project     = var.project_name
  region      = var.region
  zone        = var.zone
  # You need to authorize locally for this to work. Use one of the methods in
  # https://www.terraform.io/docs/providers/google/guides/provider_reference.html#credentials-1
}

resource "google_compute_instance" "gce_instance" {
  labels = {
    "container-vm" = "cos-stable-78-12499-59-0"
  }
  machine_type = "f1-micro"
  metadata = {
    "gce-container-declaration" = <<-EOT
        spec:
          containers:
            - name: calendar-micro
              image: 'gcr.io/${var.project_name}/goout-calendar:${var.image_tag}'
              stdin: false
              tty: false
          restartPolicy: Always

        # This container declaration format is not public API and may change without notice. Please
        # use gcloud command-line tool or Google Cloud Console to run Containers on Google Compute Engine.
    EOT
    "google-logging-enabled"    = "true"
  }
  name = "calendar-micro"
  tags = [
    "http-server",
    "https-server",
  ]
  boot_disk {
    initialize_params {
      image = "https://www.googleapis.com/compute/beta/projects/cos-cloud/global/images/cos-stable-78-12499-59-0"
    }
  }
  network_interface {
    access_config {}
  }
  service_account {
    scopes = [
      "https://www.googleapis.com/auth/devstorage.read_only",
      "https://www.googleapis.com/auth/logging.write",
      "https://www.googleapis.com/auth/monitoring.write",
      "https://www.googleapis.com/auth/service.management.readonly",
      "https://www.googleapis.com/auth/servicecontrol",
      "https://www.googleapis.com/auth/trace.append",
    ]
  }
}
