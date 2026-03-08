# fixtures/nginx

Nginx config fixture file for `.scm` query tests.

- `nginx.conf` — defines `events`, `http`, `upstream backend`, `upstream static`, and two `server` blocks (HTTP redirect and HTTPS with SSL); location blocks for `/`, `/api`, `/static`, `~ \.php$`.
