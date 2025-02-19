#![allow(unused)]

struct Watcher {

}

use std::{os::fd::AsFd, pin::Pin, sync::Arc, sync::LazyLock};
use core::{num::NonZero, sync::atomic::{AtomicUsize, Ordering}};

use futures_util::SinkExt;
use tokio_stream::StreamExt;
use tokio_serde::{formats::SymmetricalBincode, Framed, SymmetricallyFramed};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio::{sync::Mutex, io::AsyncWriteExt, net::{unix::{OwnedReadHalf, OwnedWriteHalf, SocketAddr}, UnixListener, UnixStream}};


use crate::util;

macro_rules! def_serde_compatibly_omissible_config_default {
    ($ident: ident, <$ty: ty> { $($def: tt)* }) => {
        pub mod $ident {
            pub static DEFAULT: std::sync::LazyLock<$ty> = std::sync::LazyLock::new(|| { $($def)* });
            pub fn is_default(value: &$ty) -> bool { *value == *DEFAULT }
            pub fn get_default() -> &'static $ty { &*DEFAULT }
            pub fn clone_default() -> $ty { DEFAULT.clone() }
        }
    };
}

def_serde_compatibly_omissible_config_default!(socket_path, <std::path::PathBuf> {
    crate::util::HOME.join("Library/Application Support/am-osx-status/ipc")
});


trait PacketIdCounterSource {}
mod s {
    pub struct Remote; impl super::PacketIdCounterSource for Remote {}
    pub struct Local; impl super::PacketIdCounterSource for Local {}
}



static PACKET_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
#[repr(transparent)]
struct PacketID<T: PacketIdCounterSource>(NonZero<usize>, core::marker::PhantomData<T>);
impl<T: PacketIdCounterSource> PacketID<T> {
    #[inline(always)]
    pub const unsafe fn from_usize(usize: usize) -> Self {
        Self({
            #[cfg(debug_assertions)]
            { NonZero::new(usize).expect("cannot create a packet ID with a value of zero") }
            #[cfg(not(debug_assertions))]
            unsafe { NonZero::new_unchecked(usize) }
        }, core::marker::PhantomData)
    }
}
impl PacketID<s::Local> {
    pub fn new() -> Self {
        let id = PACKET_ID_COUNTER.fetch_add(1, Ordering::AcqRel);
        unsafe { Self::from_usize(id) }
    }
}



// /// Initial packet. Transmission format is more stable & controllable; of particular importance is the version being the first, to test for compatibility.
// struct InitialPacket {
//     version: IpcVersion
// }
// impl InitialPacket {
//     /// Returns the byte length of the packet of the version, including the version itself.
//     pub const fn byte_length(version: IpcVersion) -> usize {
//         core::mem::size_of::<IpcVersion>()
//     }
// }

#[derive(Debug)]
pub struct DuplicateReceiverError;
impl core::error::Error for DuplicateReceiverError {}
impl core::fmt::Display for DuplicateReceiverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("duplicate receiver")
    }
}


// #[derive(Debug, thiserror::Error)]
// enum ReceiverCreationError {
//     #[error("{0}")]
//     DuplicateReceiver(#[from] DuplicateReceiverError),
//     #[error("sender has an incompatible version")]
//     SenderIncompatibleVersion,
//     #[error("first packet received was non-hello")]
//     NoHello,
//     #[error("timed out (did not receive hello)")]
//     TimedOut,
// }


const IPC_VERSION: usize = 0;

pub mod packets {
    use crate::util::OWN_PID;

    use super::IPC_VERSION;

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    pub struct Hello {
        pub version: usize,
        pub process: libc::pid_t
    }
    impl Hello {
        pub fn new() -> Self {
            Hello {
                version: IPC_VERSION,
                process: *OWN_PID
            }
        }
    }
    impl From<Hello> for super::Packet {
        fn from(val: Hello) -> Self {
            super::Packet::Hello(val)
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[repr(u16)]
pub enum Packet {
    Hello(packets::Hello) = 0,
    ReloadConfiguration = 1,
}
impl Packet {
    pub fn hello() -> Self {
        packets::Hello::new().into()
    }
}

pub struct Listener {
    address: SocketAddr,
    closed: bool,
    new_connection: tokio::sync::mpsc::Receiver<UnixStream>
}
impl Listener {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, DuplicateReceiverError> {
        let listener = match UnixListener::bind(path) {
            Ok(listener) => Ok(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => Err(DuplicateReceiverError),
            Err(err) => panic!("cannot create receiver: {:?}", err)
        }?;

        let address = listener.local_addr().expect("no local address for ipc socket");
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        
        tokio::spawn(async move {
            loop {
                tx.send(listener.accept().await.expect("unable to accept connection").0).await.unwrap();
            }
        });
        
        Ok(Self {
            address,
            closed: false,
            new_connection: rx
        })
    }
    async fn next_connection(&mut self) -> PacketConnection {
        PacketConnection::from_stream(self.new_connection.recv().await.expect("channel closed")).await
    }
    pub fn shutdown(&mut self) {
        if self.closed { return }
        let path = self.address.as_pathname().expect("ipc socket is unnamed");
        std::fs::remove_file(path).expect("cannot remove IPC socket");
        self.closed = true;
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug)]
pub struct PacketConnection {
    outgoing: Framed<
        FramedWrite<
            OwnedWriteHalf,
            LengthDelimitedCodec
        >,
        Packet,
        Packet,
        SymmetricalBincode<Packet>
    >,

    incoming: Framed<
        FramedRead<
            OwnedReadHalf,
            LengthDelimitedCodec
        >,
        Packet,
        Packet,
        SymmetricalBincode<Packet>
    >
}
impl PacketConnection {
    pub async fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        Ok(Self::from_stream(tokio::net::UnixStream::connect(path).await?).await)
    }

    pub async fn from_stream(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();

        let framed_write = FramedWrite::new(write, LengthDelimitedCodec::new());
        let framed_read = FramedRead::new(read, LengthDelimitedCodec::new());

        let mut outgoing = SymmetricallyFramed::new(framed_write, SymmetricalBincode::<Packet>::default());
        let mut incoming = SymmetricallyFramed::new(framed_read, SymmetricalBincode::<Packet>::default());

        Self { outgoing, incoming }
    }

    pub async fn recv(&mut self) -> Result<Option<Packet>, std::io::Error> {
        self.incoming.next().await.transpose()
    }

    pub async fn send(&mut self, packet: impl Into<Packet>) -> Result<(), std::io::Error> {
        self.outgoing.send(packet.into()).await?;
        Ok(())
    }
}

pub async fn listen(
    context: Arc<Mutex<crate::PollingContext>>,
    config: Arc<Mutex<crate::config::Config<'static>>>
) -> Arc<Mutex<Listener>> {
    let mut listener = Listener::new({ config.clone().lock().await.socket_path.to_owned() }).unwrap();
    let listener = Arc::new(Mutex::new(listener));
    let listener_sent = listener.clone();
    let config = config.clone();

    tokio::spawn(async move {
        loop {
            let mut connection = { listener_sent.lock().await }.next_connection().await;
            let context = context.clone();
            let config = config.clone();
           
            tokio::spawn(async move {
                let hello = connection.recv().await.unwrap().expect("no hello!?");
                let hello = if let Packet::Hello(hello) = hello { hello } else { panic!("wanted hello first") };

                loop {
                    while let Some(packet) = connection.recv().await.expect("shit") {
                        match packet {
                            Packet::Hello(..) => panic!("no double hello"),
                            Packet::ReloadConfiguration => {
                                let mut config = config.lock().await;
                                config.reload_from_disk().await.expect("could not update config");
                                context.lock().await.reload_from_config(&config).await;
                            }
                        }
                    }
                }
            });
        };
    });

    listener
}

