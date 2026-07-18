FROM python:3.14-slim

WORKDIR /app

COPY pylos/memory/requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY pylos/memory/ ./pylos/memory/

EXPOSE 7577

ENV PYTHONPATH=/app

CMD ["python", "-m", "pylos.memory.server"]
