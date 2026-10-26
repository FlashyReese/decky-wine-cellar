# Decky's SteamOS Rust toolchain, pinned to keep release builds reproducible.
# Includes Rust 1.94.0; update the digest deliberately when upgrading the toolchain.
FROM ghcr.io/steamdeckhomebrew/holo-toolchain-rust@sha256:818de45e147f35798c66498380b31bd1fe8bf75afdf0a22b027053706836faf1

# entrypoint.sh should always be located in the backend folder
ENTRYPOINT [ "/backend/entrypoint.sh" ]
