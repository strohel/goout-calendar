function inputChanged() {
    var value = document.getElementById("id").value;
    if (value == "") {
        setUrls("", "");
        return;
    }

    try {
        var id = convertTextToId(value)
    }
    catch(err) {
        var msg = "Invalid textual ID '" + value + "': " + err
        setUrls(msg, msg);
        return;
    }

    // webcal://goout.net/services/feeder/usercalendar.ics?id=43224?...
    var http_url = window.location.href + "services/feeder/usercalendar.ics?id=" + id;

    var language_value = document.getElementById("language").value;
    http_url += "&language=" + language_value;

    var after_value = document.getElementById("after").value;
    if (after_value) {
        http_url += "&after=" + after_value;
    }

    var webcal_url = http_url.replace(/^https?/, "webcal");
    setUrls(http_url, webcal_url);
}

function convertTextToId(value) {
    var base = 25;
    var base_char_code = 'a'.charCodeAt(0);

    var id = 0;
    for (var i = 0; i < value.length; i++) {
        var code = value.charCodeAt(i) - base_char_code;
        if (code < 0 || code >= base) {
            throw "Invalid character " + value.charAt(i);
        }
        id += code * Math.pow(base, i);
    }
    return id;
}

function setUrls(http_url, webcal_url) {
    var entries = {
        http: http_url,
        webcal: webcal_url
    };

    for (var prop in entries) {
        var url = entries[prop];

        var input_elem = document.getElementById("input-" + prop);
        var a_elem = document.getElementById("a-" + prop);

        input_elem.value = url;
        a_elem.href = url;
    }
}
