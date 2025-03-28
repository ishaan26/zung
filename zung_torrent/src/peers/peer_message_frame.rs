use crate::peers::PeerMessagesTag;

use super::{PeerMessage, PeerMessagePayload, UnchokePayload};
use anyhow::{anyhow, ensure, Result};
use bytes::BytesMut;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};

// TODO: Check is using async_trait is ok or not.
#[async_trait::async_trait]
pub trait PeerMessageFrame: AsyncRead + AsyncWrite + Unpin {
    async fn recv_peer_message<T>(&mut self, buf: &mut BytesMut) -> Result<PeerMessage<T>>
    where
        T: PeerMessagePayload,
    {
        loop {
            let read = self.read_buf(buf).await?;

            if read == 0 {
                continue;
            }

            if buf.len() < 4 {
                tracing::error!("Peer sent less than 4 bytes");
                continue;
            }

            let len = u32::from_be_bytes(buf[0..4].try_into()?);

            if len as usize > buf.len() {
                tracing::error!(
                    "Peer sent insuffecient data, expected: {len}, recv: {}",
                    buf.len()
                );

                continue;
            }

            ensure!(len as usize <= (buf.len() - 4));

            let tag = PeerMessagesTag::try_from(buf[4]).map_err(|e| anyhow!(e))?;

            ensure!(tag == T::message_tag());

            let payload = T::from_bytes(&buf[5..(5 + len - 1) as usize])?;

            buf.truncate(len as usize + 4);

            let message: PeerMessage<T> = PeerMessage::new(len, tag, payload);

            break Ok(message);
        }
    }

    async fn recv_unchoke_message(&mut self) -> Result<PeerMessage<UnchokePayload>> {
        let mut buf = [0; 5];
        self.read_exact(&mut buf).await?;
        PeerMessage::from_bytes(&buf)
    }

    async fn send_message<T>(&mut self, message: PeerMessage<T>) -> Result<()>
    where
        T: PeerMessagePayload + Send,
    {
        let written = self.write(&message.to_bytes()).await?;
        ensure!(message.size() == written);
        Ok(())
    }
}

#[async_trait::async_trait]
impl PeerMessageFrame for TcpStream {}
