
struct Watcher {

}

use std::{os::fd::AsFd, sync::LazyLock};

use tokio::io::AsyncWriteExt;

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
    crate::util::HOME.join("Application Support/am-osx-status/ipc")
});


trait PacketIdCounterSource {}
mod s {
    pub struct Remote; impl super::PacketIdCounterSource for Remote {}
    pub struct Local; impl super::PacketIdCounterSource for Local {}
}

use core::{num::NonZero, sync::atomic::{AtomicUsize, Ordering}};


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

#[derive(serde::Serialize, serde::Deserialize)]
#[repr(u16)]
pub enum Packet {
    Hello { version: usize } = 0,
    Restart,
}


pub struct PacketReceiver {
    address: tokio::net::unix::SocketAddr,
    closed: bool,
    rx: tokio::sync::mpsc::Receiver<Packet>
}
impl PacketReceiver {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, DuplicateReceiverError> {
        let listener = match tokio::net::UnixListener::bind(path) {
            Ok(listener) => Ok(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => Err(DuplicateReceiverError),
            Err(err) => panic!("cannot create receiver: {:?}", err)
        }?;


        
        let address = listener.local_addr().expect("no local address for ipc socket");
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        
        tokio::spawn(async move {
            loop {
                let mut stream = listener.accept().await.expect("unable to accept connection").0;
                let tx = tx.clone();

                tokio::spawn(async move {
                    use tokio_stream::StreamExt;
                    use tokio_serde::*;
                    use tokio_serde::formats::SymmetricalBincode;
                    use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

                    let framed = FramedRead::new(stream, LengthDelimitedCodec::new());
                    let mut deserialized = tokio_serde::SymmetricallyFramed::new(
                        framed,
                        SymmetricalBincode::<Packet>::default(),
                    );

                    // TODO: Handle deserialization failure
                    while let Some(packet) = deserialized.try_next().await.unwrap() {
                        tx.send(packet).await.expect("receiver closed");
                    }
                });
            }
        });
        
        Ok(Self {
            address,
            closed: false,
            rx
        })
    }
    pub async fn next(&mut self) -> Packet {
        self.rx.recv().await.expect("channel closed")
    }
    fn shutdown(&mut self) {
        if self.closed { return }
        let path = self.address.as_pathname().expect("ipc socket is unnamed");
        std::fs::remove_file(path).expect("cannot remove IPC socket");
        self.closed = true;
    }
}
