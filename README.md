# Envoi - Rust based reverse proxy

## Planned features

### Different SSL certs per host

Have one main router, and then send the request to the other router? This would be one sub router per cert...

### Configuration

Would start with a toml config that would later evolve into a web config.
This would allow creating new routes, mapping host(s) => destination(s), as well as an ssl config to use for that host.
Automatical.

### Speed

Should be faster than NGINX, simply by being built directly on top of hyper, and limiting the scope.

## tls

convert using openssl cli to .der (b64 format).


PKCS8 => PKCS1 (-----BEGIN PRIVATE KEY-----)
=> `openssl pkey  -in input.pem -out output.key -traditional`

EC => DER (-----BEGIN EC PRIVATE KEY-----)
=> `openssl ec -in input.key -out output.der -outform DER`