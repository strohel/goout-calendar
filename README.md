# goout-calendar

Simple web service that serves calendar of a goout.net user in iCalendar format.

## Documentation

Reverse-engineered GoOut REST API documentation is present in this repository
in the [apiary.apib](apiary.apib) file in
[API Blueprint](https://apiblueprint.org/) format, and [beautifully rendered
(including ability to call endpoints) on Apiary](https://strohel.docs.apiary.io/).

The [endpoints.txt](endpoints.txt) file simply lists available endpoints.

Goout-calendar itself is a [Rust](https://www.rust-lang.org/) micro web service
using [Rocket](https://rocket.rs/) as a web framework. The
[calendar.rs](src/calendar.rs) module is responsible for handling of the
client-facing endpoint, which lets the [goout_api.rs](src/goout_api.rs) module
iteract with GoOut API.

## Build and Deploy

Goout-calendar requires Rust 1.33+, but the Rocket dependency
[requires nightly or devel Rust build](https://github.com/SergioBenitez/Rocket/issues/19).

If you install Rust toolchain locally, you can `cargo build`, `cargo run` etc.

Alternatively, enclosed [Dockerfile](Dockerfile) lets you build using
`docker build -t goout-calendar .` and run using
`docker run -p 80:80 goout-calendar`.

You can also deploy to any [Kubernetes](https://kubernetes.io/) cluster by
substituting the `$IMAGE_TAG` variable in [kubernetes.envsubst.yaml](kubernetes.envsubst.yaml)
(producing `kubernetes.yaml` manifest file) and deploying the `Service` and
`Deployment` using e.g. `kubectl apply -f kubernetes.yaml`.

Finally, there is [cloudbuild.yaml](cloudbuild.yaml) configuration file for
[Google Cloud Build (Continuous Integration)](https://cloud.google.com/cloud-build/)
to build and deploy automatically. *Note to myself: I've configured GCB trigger
on the GitHub repo to build & deploy automatically on push of a tag.*

## Demo Instance

I've set up a demo instance at [Google Cloud Platform](https://cloud.google.com/)
at <http://goout.strohel.eu/>.

## See Also

* <https://goout.net/>
* <https://github.com/agiertli/goout-stalker> for a somewhat similar project, and
  [an associated blog post](https://medium.com/@respectx/ed65391836f3).
* There is also a *UserScript*
  [Gist to Add Event to Google Calendar on GoOut.net](https://gist.github.com/jnv/b1891f33fb7b6f6d03dd435ba7dc3266).
