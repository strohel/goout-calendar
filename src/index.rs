use rocket::{get, response::content};

const INDEX_HTML: &str = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <link rel="stylesheet"
              href="https://cdnjs.cloudflare.com/ajax/libs/bulma/0.7.2/css/bulma.min.css">
        <title>GoOut Calendar Exporter</title>
    </head>
    <body>
    <section class="section">
        <div class="container">
            <h1 class="title">GoOut Calendar Exporter</h1>
            <p class="subtitle">
                This is a simple proxy Web service that serves events of a given
                user of <a href="https://goout.net/">GoOut.net</a> in an iCalendar
                format. See
                <a href="https://github.com/strohel/goout-calendar">homepage
                on GitHub</a> for more info and source code.
            </p>
        </div>
    </section>
    </body>
    </html>
"#;

#[get("/")]
pub(in crate) fn serve() -> content::Html<&'static str> {
    content::Html(INDEX_HTML)
}
