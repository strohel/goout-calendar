provider "google" {
  version     = "~> 3.3"
  project     = var.project_name
  region      = var.region
  zone        = var.zone
  # You need to authorize locally for this to work. Use one of the methods in
  # https://www.terraform.io/docs/providers/google/guides/provider_reference.html#credentials-1
}

resource "google_container_cluster" "gke_cluster" {
  default_max_pods_per_node   = 110
  enable_binary_authorization = false
  enable_intranode_visibility = false
  enable_kubernetes_alpha     = false
  enable_legacy_abac          = false
  enable_tpu                  = false
  initial_node_count          = 0
  location                    = "us-east1-b"
  logging_service             = "logging.googleapis.com/kubernetes"
  monitoring_service          = "monitoring.googleapis.com/kubernetes"
  name                        = "goout-calendar"
  network                     = "projects/strohel-goout-calendar/global/networks/default"
  node_locations              = []
  node_version                = "1.12.10-gke.17"
  project                     = "strohel-goout-calendar"
  resource_labels             = {}
  subnetwork                  = "projects/strohel-goout-calendar/regions/us-east1/subnetworks/default"

  addons_config {

    http_load_balancing {
      disabled = false
    }

    istio_config {
      disabled = true
    }

    network_policy_config {
      disabled = true
    }
  }

  cluster_autoscaling {
    enabled = false
  }

  database_encryption {
    state = "DECRYPTED"
  }

  ip_allocation_policy {
    cluster_ipv4_cidr_block  = "10.44.0.0/14"
    services_ipv4_cidr_block = "10.0.16.0/20"
  }

  master_auth {

    client_certificate_config {
      issue_client_certificate = false
    }
  }

  network_policy {
    enabled = false
  }

  node_config {
    disk_size_gb      = 10
    disk_type         = "pd-standard"
    guest_accelerator = []
    image_type        = "COS"
    labels            = {}
    local_ssd_count   = 0
    machine_type      = "g1-small"
    metadata = {
      "disable-legacy-endpoints" = "true"
    }
    oauth_scopes = [
      "https://www.googleapis.com/auth/devstorage.read_only",
      "https://www.googleapis.com/auth/logging.write",
      "https://www.googleapis.com/auth/monitoring",
      "https://www.googleapis.com/auth/service.management.readonly",
      "https://www.googleapis.com/auth/servicecontrol",
      "https://www.googleapis.com/auth/trace.append",
    ]
    preemptible     = false
    service_account = "default"
    tags            = []
    taint           = []
  }

  node_pool {
    initial_node_count = 1
    max_pods_per_node  = 110
    name               = "pool-1"
    node_count         = 1
    node_locations     = []
    version            = "1.12.10-gke.17"

    management {
      auto_repair  = true
      auto_upgrade = true
    }

    node_config {
      disk_size_gb      = 10
      disk_type         = "pd-standard"
      guest_accelerator = []
      image_type        = "COS"
      labels            = {}
      local_ssd_count   = 0
      machine_type      = "g1-small"
      metadata = {
        "disable-legacy-endpoints" = "true"
      }
      oauth_scopes = [
        "https://www.googleapis.com/auth/devstorage.read_only",
        "https://www.googleapis.com/auth/logging.write",
        "https://www.googleapis.com/auth/monitoring",
        "https://www.googleapis.com/auth/service.management.readonly",
        "https://www.googleapis.com/auth/servicecontrol",
        "https://www.googleapis.com/auth/trace.append",
      ]
      preemptible     = false
      service_account = "default"
      tags            = []
      taint           = []
    }
  }

  release_channel {
    channel = "UNSPECIFIED"
  }

  timeouts {}
}
