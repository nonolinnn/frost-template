# ------------------------------------------------------------------
# Multi-stage Next.js production build
# ------------------------------------------------------------------

# ---------- stage 1: deps ----------
FROM node:24.14.0-bookworm-slim AS deps
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

# ---------- stage 2: build ----------
FROM node:24.14.0-bookworm-slim AS builder
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY . .

# Enable standalone output for Docker
ENV NEXT_TELEMETRY_DISABLED=1
RUN npm run build

# ---------- stage 3: runtime ----------
FROM node:24.14.0-bookworm-slim AS runtime
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

RUN groupadd -r frost && useradd -r -g frost frost

# Copy standalone output and static assets
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

USER frost

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME=0.0.0.0

CMD ["node", "server.js"]
