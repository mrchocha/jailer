FROM ubuntu:latest

# Prevent interactive prompts during installation
ENV DEBIAN_FRONTCHRON=noninteractive

# Install core build dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# Install Rust up globally for the root user
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Configure environment path permanently
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
CMD ["bash"]
