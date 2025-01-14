use actix_web::{get, web, App, HttpRequest, HttpServer, HttpResponse, Responder};
use reqwest::Client;
use serde::Deserialize;
use std::env;
use chrono::Utc;
use futures::future::join_all;

#[derive(Deserialize)]
#[serde(untagged)]
enum SyncingStatus {
    Bool(bool),
    Object {
        #[serde(rename = "currentBlock")]
        current_block: String,

        #[serde(rename = "healedBytecodeBytes")]
        healed_bytecode_bytes: String,

        #[serde(rename = "healedBytecodes")]
        healed_bytecodes: String,

        #[serde(rename = "healedTrienodeBytes")]
        healed_trienode_bytes: String,

        #[serde(rename = "healedTrienodes")]
        healed_trienodes: String,

        #[serde(rename = "healingBytecode")]
        healing_bytecode: String,

        #[serde(rename = "healingTrienodes")]
        healing_trienodes: String,

        #[serde(rename = "highestBlock")]
        highest_block: String,

        #[serde(rename = "startingBlock")]
        starting_block: String,

        #[serde(rename = "syncedAccountBytes")]
        synced_account_bytes: String,

        #[serde(rename = "syncedAccounts")]
        synced_accounts: String,

        #[serde(rename = "syncedBytecodeBytes")]
        synced_bytecode_bytes: String,

        #[serde(rename = "syncedBytecodes")]
        synced_bytecodes: String,

        #[serde(rename = "syncedStorage")]
        synced_storage: String,

        #[serde(rename = "syncedStorageBytes")]
        synced_storage_bytes: String,

        #[serde(rename = "txIndexFinishedBlocks")]
        tx_index_finished_blocks: String,

        #[serde(rename = "txIndexRemainingBlocks")]
        tx_index_remaining_blocks: String,
    },
}

#[derive(Deserialize)]
#[allow(dead_code)] // suppress warnings for some unused fields
struct SyncingResponse {
    #[serde(skip)]
    jsonrpc: String,
    #[serde(skip)]
    id: u64,
    result: SyncingStatus,
}

#[derive(Deserialize)]
#[allow(dead_code)] // suppress warnings for some unused fields
struct BlockNumber {
    #[serde(skip)]
    jsonrpc: String,
    #[serde(skip)]
    id: u64,
    result: String,
}

async fn get_block_number(client: &Client, url: &str) -> Result<u64, reqwest::Error> {
    let payload = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;
    let response = client.post(url)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .send()
        .await?;
    let block_number: BlockNumber = response.json().await?;
    Ok(u64::from_str_radix(&block_number.result.trim_start_matches("0x"), 16).unwrap())
}

#[get("/health")]
async fn health_check(req: HttpRequest, client: web::Data<Client>, ethereum_node_url: web::Data<String>, reference_nodes: web::Data<Vec<String>>) -> impl Responder {
    let url = ethereum_node_url.get_ref();
    let payload = r#"{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":0}"#;

    // Get current timestamp with specific format
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

    // Get client IP address, host, and request path
    let client_ip = req.headers().get("X-Forwarded-For")
        .and_then(|header| header.to_str().ok())
        .map(String::from)
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "Unknown".to_string());

    let client_host = req.connection_info().host().to_string();
    let request_path = req.path();

    println!("{} - Sending request to Ethereum node at {} from Client IP: {}, Host: {}, Request Path: {}", timestamp, url, client_ip, client_host, request_path);

    // Get head block number from the Ethereum node
    let own_block_number = get_block_number(&client, url).await;

    // Get head block numbers from reference nodes
    let reference_block_numbers: Vec<_> = reference_nodes.iter()
        .map(|node_url| get_block_number(&client, node_url))
        .collect();
    let reference_block_numbers = join_all(reference_block_numbers).await;

    let valid_block_numbers: Vec<(String, u64)> = reference_nodes.iter()
        .zip(reference_block_numbers)
        .filter_map(|(node_url, result)| result.ok().map(|block_number| (node_url.clone(), block_number)))
        .collect();

    let max_reference_block_number = valid_block_numbers.iter().map(|&(_, num)| num).max().unwrap_or(0);

    match own_block_number {
        Ok(own_number) => {
            let block_diff = if max_reference_block_number > own_number {
                max_reference_block_number - own_number
            } else {
                own_number - max_reference_block_number
            };

            println!("Own block number: {}", own_number);
            println!("Max reference block number: {}", max_reference_block_number);

            for (node_url, block_number) in valid_block_numbers {
                println!("{} block number: {}", node_url, block_number);
            }

            match client.post(url)
                .header("Content-Type", "application/json")
                .body(payload.to_string())
                .send()
                .await {
                Ok(response) => {
                    match response.json::<SyncingResponse>().await {
                        Ok(sync_response) => {
                            match sync_response.result {
                                SyncingStatus::Bool(is_syncing) => {
                                    if is_syncing {
                                        HttpResponse::ServiceUnavailable().body("Ethereum node is syncing")
                                    } else if block_diff >= 10 {
                                        HttpResponse::ServiceUnavailable().body("Ethereum node is behind reference nodes")
                                    } else {
                                        HttpResponse::Ok().body("Ethereum node is healthy")
                                    }
                                }
                                SyncingStatus::Object { .. } => {
                                    HttpResponse::ServiceUnavailable().body("Ethereum node is syncing")
                                }
                            }
                        }
                        Err(e) => {
                            println!("Failed to parse Ethereum node status: {}", e);
                            HttpResponse::InternalServerError().body("Failed to parse Ethereum node status")
                        },
                    }
                }
                Err(e) => {
                    println!("Failed to reach Ethereum node: {}", e);
                    HttpResponse::BadGateway().body("Failed to reach Ethereum node")
                },
            }
        }
        Err(e) => {
            println!("Failed to reach Ethereum node: {}", e);
            HttpResponse::BadGateway().body("Failed to reach Ethereum node")
        },
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let ethereum_node_url = env::var("ETHEREUM_NODE_URL").expect("ETHEREUM_NODE_URL must be set");
    let reference_nodes: Vec<String> = env::var("REFERENCE_NODES").expect("REFERENCE_NODES must be set")
        .split(',')
        .map(String::from)
        .collect();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Client::new()))
            .app_data(web::Data::new(ethereum_node_url.clone()))
            .app_data(web::Data::new(reference_nodes.clone()))
            .service(health_check)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}