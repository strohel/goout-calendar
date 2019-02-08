use rocket::{get, response::content};

const INDEX_HTML: &str = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="utf-8">
        <title>GoOut Calendar Exporter</title>
    </head>
    <body>
        <h1>GoOut Calendar Exporter</h1>
        <p>
            This is a simple proxy Web service that serves events of a given
            user of <a href="https://goout.net/">GoOut.net</a> in an iCalendar
            format. See
            <a href="https://github.com/strohel/goout-calendar">homepage
            on GitHub</a> for more info and source code.
        </p>
    </body>
    </html>
"#;

#[get("/")]
pub(in crate) fn serve() -> content::Html<&'static str> {
    content::Html(INDEX_HTML)
}
