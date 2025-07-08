use anyhow::Result;
use solana_gossip::{
    cluster_info::{ClusterInfo, Node},
    contact_info::ContactInfo,
    gossip_service::GossipService,
};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tracing::{info, error};

pub struct P2PNode {
    keypair: Arc<Keypair>,
    cluster_info: Arc<ClusterInfo>,
    gossip_service: Option<GossipService>,
}

impl P2PNode {
    pub fn new(
        keypair: Keypair,
        entrypoints: Vec<SocketAddr>,
        bind_address: SocketAddr,
    ) -> Result<Self> {
        let keypair = Arc::new(keypair);
        let node_pubkey = keypair.pubkey();
        
        // Create node identity
        let node = Node::new_localhost_with_pubkey(&node_pubkey);
        
        // Create contact info
        let contact_info = ContactInfo::new_localhost(&node_pubkey, 0);
        
        // Initialize cluster info
        let cluster_info = Arc::new(ClusterInfo::new(
            contact_info,
            keypair.clone(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
        ));
        
        // Set entrypoints
        cluster_info.set_entrypoints(entrypoints);
        
        Ok(Self {
            keypair,
            cluster_info,
            gossip_service: None,
        })
    }
    
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting P2P node with pubkey: {}", self.keypair.pubkey());
        
        // Start gossip service
        let (gossip_service, _gossip_socket) = GossipService::new(
            &self.cluster_info,
            None, // bank_forks
            self.cluster_info.socket_addr_space(),
            None, // gossip_validators
            None, // should_check_duplicate_instance
            None, // stats_reporter_sender
            solana_streamer::socket::SocketAddrSpace::Unspecified,
        )?;
        
        self.gossip_service = Some(gossip_service);
        
        info!("Gossip service started");
        
        // Monitor cluster
        self.monitor_cluster().await?;
        
        Ok(())
    }
    
    async fn monitor_cluster(&self) -> Result<()> {
        loop {
            let all_peers = self.cluster_info.all_peers();
            info!("Connected to {} peers", all_peers.len());
            
            for peer in all_peers.iter().take(5) {
                info!("Peer: {} at {}", peer.id, peer.gossip);
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    }
    
    pub fn get_cluster_nodes(&self) -> Vec<ContactInfo> {
        self.cluster_info.all_peers()
    }
    
    pub fn get_node_pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
} 