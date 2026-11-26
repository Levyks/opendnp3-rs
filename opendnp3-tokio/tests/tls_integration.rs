//! End-to-end TLS integration test: Tokio TLS Master <-> Tokio TLS Outstation.

#![cfg(feature = "tls")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use opendnp3::app::{Binary, HeaderInfo, Indexed};
use opendnp3::enums::{EventMode, MasterTaskType, TaskCompletion};
use opendnp3::link::LinkConfig;
use opendnp3::master::{DefaultMasterApplication, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, OutstationConfig,
};
use opendnp3_tokio::{spawn_tls_master, spawn_tls_outstation};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::{TlsAcceptor, TlsConnector};

struct SharedSoe {
    binaries: Arc<Mutex<Vec<Indexed<Binary>>>>,
}

impl SoeHandler for SharedSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.lock().unwrap().extend_from_slice(values);
    }
}

#[tokio::test]
async fn tls_master_and_outstation_end_to_end() {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    // Generate ephemeral self-signed certificate for localhost
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert);
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();

    let server_cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(server_cfg));

    let mut roots = RootCertStore::empty();
    roots.add(cert_der).unwrap();
    let client_cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(client_cfg));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut o_link = LinkConfig::new(false);
    o_link.local_addr = 10;
    o_link.remote_addr = 1;
    let o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let o_handle = spawn_tls_outstation(
        listener,
        acceptor,
        o_link,
        o_cfg,
        DefaultCommandHandler,
        DefaultOutstationApplication,
    );
    o_handle.update_binary(1, Binary::new(true), EventMode::Suppress);

    let mut m_link = LinkConfig::new(true);
    m_link.local_addr = 1;
    m_link.remote_addr = 10;
    let binaries = Arc::new(Mutex::new(Vec::new()));
    let (m_handle, ev_rx) = spawn_tls_master(
        addr,
        ServerName::try_from("localhost").unwrap(),
        connector,
        m_link,
        MasterParams::default(),
        SharedSoe {
            binaries: binaries.clone(),
        },
        DefaultMasterApplication,
    )
    .await
    .unwrap();

    timeout(Duration::from_secs(5), async {
        while let Some(ev) = ev_rx.recv().await {
            if matches!(
                ev,
                MasterEvent::TaskComplete {
                    task_type: MasterTaskType::StartupIntegrityPoll,
                    result: TaskCompletion::Success,
                }
            ) {
                break;
            }
        }
    })
    .await
    .expect("TLS integrity poll timed out");

    assert!(
        binaries
            .lock()
            .unwrap()
            .iter()
            .any(|b| b.index == 1 && b.value.value)
    );

    m_handle.shutdown();
    o_handle.shutdown();
}
