# Étape 1 : Image de build avec Python et les dépendances
FROM python:3.14-slim AS builder

WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Étape 2 : Image distroless minimale
FROM gcr.io/distroless/python3-debian12:nonroot

WORKDIR /app
COPY --from=builder /usr/local/lib/python3.12/site-packages /usr/local/lib/python3.12/site-packages
COPY server.py .

EXPOSE 8000

CMD ["server.py"]
