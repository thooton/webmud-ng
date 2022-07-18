# webmud-ng
This is a modern fork of the unmaintained project [PHudBase-WebMud](https://web.archive.org/web/20201112015926/https://phudbase.com/webmud.php) ([src](https://code.google.com/archive/p/phudbase/)) with the backend rewritten in Rust. 

The goal is to provide a simple interface for a web user to interact with any [MUD](https://en.wikipedia.org/wiki/MUD).

## Features
- Single, statically linked binary with zero dependencies.
- Supports both modern TLS and unencrypted connections.
- Supports both modern and legacy ([2010 IETF draft](https://web.archive.org/web/20100607025404/http://www.ietf.org/id/draft-ietf-hybi-thewebsocketprotocol-00.txt)) WebSocket protocols.
- Security by default: web clients cannot connect to loopback interfaces or private address space, and TLS certificates are verified. `eval` is not used. HTML from server is sanitized with js-xss.

## Caveats
- Sidebar in the old project is removed, which included connection status indicators as well as clickable buttons for movement. This should not be very hard to add back if you want it.
- The Flash client (for browsers that do not support WebSockets) is still untested. If you test it and it doesn't work, feel free to open an issue.

## Installation
Download the appropriate binary from the releases section. Put it in your PATH if you like.

## Examples
`webmud-ng <listen ip> <listen port>` - Listen on `http://<listen ip>:<listen port>` as well as `ws://<listen ip>:<listen port>/ws`. This should suit most use-cases. Note that `<listen ip>` should be set to `0.0.0.0` if you wish to allow external connections and `127.0.0.1` otherwise. You can connect to the IP and port as-is or set up your favorite HTTP reverse proxy (e.g. Nginx) to point to it. With a reverse proxy, HTTPS/WSS should work without any issues.

`webmud-ng <listen ip> <listen port> --serve-from=<path>` - For ease of use, the client web files are bundled with the executable. If you would like to make changes without recompiling, then download the `static` folder from this repository and set `<path>` to its path.

`webmud-ng <listen ip> <listen port> --legacy-ip=<legacy listen ip> --legacy-port=<legacy listen port>` - This starts a listener for legacy WebSocket connections bound to `ws://<legacy listen ip>:<legacy listen port>`. Legacy clients will attempt connections to `ws://<hostname in URL>:<legacy listen port>`. If you need legacy clients to connect to a different host or port, then consider using the options `--legacy-extern-host=#` and `--legacy-extern-port=#`. If you need legacy clients to connect over TLS, then use `--legacy-extern-is-https`.

## Todo
- Improve legacy WebSocket client detection (currently counts number of keys in `WebSocket.prototype` and compares it to a certain threshold).
- ~~Do real parsing of Telnet colors instead of using regex.~~

## Usage
`webmud-ng <ip> <port> [--extern-is-https] [--legacy-only] [--legacy-ip=#] [--legacy-port=#] [--legacy-extern-host=#] [--legacy-extern-port=#] [--legacy-extern-is-https] [--no-color] [--serve-from=directory] [--allow-private-connections] [--allow-invalid-tls] [--debug]`

`ip` - Required. The local IP for the web server and modern WS server to bind to.

`port` - Required. The local port for the web server and modern WS server to bind to.

`--extern-is-https` - If the default `ws://` is causing modern WebSocket clients to connect without TLS when they should, then this flag will force the prefix to `wss://`. This should not be required on newer browsers.

`--legacy-only` - No web server. Listen for legacy WebSocket connections only.

`--legacy-ip=#` - Set the IP for the legacy WebSocket listener to bind to.

`--legacy-port=#` - Set the port for the legacy WebSocket listener to bind to.

`--legacy-extern-host=#` - Set the external host that legacy WebSocket clients will attempt to connect to.

`--legacy-extern-port=#` - Set the external port that legacy WebSocket clients will attempt to connect to.

`--legacy-extern-is-https` - Legacy WebSocket clients will use the prefix `wss://` instead of `ws://`.

`--no-color` - Instead of replacing Telnet colors with HTML/CSS equivalents, strip all color entirely.

`--serve-from=#` - Serve web server files dynamically from the directory `#`.

`--allow-private-connections` - Allow clients to connect to loopback/private ranges.

`--allow-invalid-tls` - Allow clients to connect via TLS, even if the certificate is invalid.

`--debug` - Print some debug info about incoming connections.

## License
My changes are licensed under CC BY-SA 4.0, but the parts retained from PHudBase-WebMud are dual-licensed under CC BY 3.0 and GNU GPL v3.
