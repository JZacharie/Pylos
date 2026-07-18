FROM node:22-alpine AS builder

WORKDIR /app

# Root package.json (proxies to ui/)
COPY package*.json ./
RUN npm ci

# UI dependencies
COPY ui/package*.json ./ui/
RUN npm ci --prefix ui

COPY . .
RUN npm run build

# ── Runtime : nginx pour servir le SPA ──────────────────────────────────────
FROM nginx:1.31 AS runtime

COPY --from=builder /app/ui/dist /usr/share/nginx/html
COPY ui/nginx.conf /etc/nginx/conf.d/default.conf

COPY ui/docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh && chown -R 101:101 /usr/share/nginx/html

EXPOSE 80

ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["nginx", "-g", "daemon off;"]
