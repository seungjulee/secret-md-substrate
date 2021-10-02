use anyhow::Result;
use log::info;
use parity_scale_codec::{Decode, Encode};
use phala_mq::MessageOrigin;
use serde::{Deserialize, Serialize};
use serde_json;

use surf;

use super::{TransactionError, TransactionResult};
use crate::contracts;
use crate::contracts::{AccountId, NativeContext};
use crate::side_task::async_side_task::AsyncSideTask;
extern crate runtime as chain;

use phala_types::messaging::BtcPriceBotCommand;

type Command = BtcPriceBotCommand;

/// Contract Overview
///
/// This contract show the ability of Phala contracts to send HTTP request with `AsyncSideTask`.
///
/// We recommend first to read about asynchronous programming in Rust <https://rust-lang.github.io/async-book/> since our
/// `AsyncSideTask` follows the same manner.
///
/// For now, you cannot use `tokio`-based HTTP crate since it is not compatible in SGX. Read more about the details in our
/// PR <https://github.com/Phala-Network/phala-blockchain/pull/483> for the reason why.
///
/// For more side task demos, visit <https://github.com/Phala-Network/phala-blockchain/tree/side-task-demo1> and
/// <https://github.com/Phala-Network/phala-blockchain/tree/side-task-demo2>.
///
/// For the basic functionalities of contract, refer to `guess_number.rs`.

pub struct BtcPriceBot {
    owner: AccountId,
    bot_token: String,
    chat_id: String,
}

/// The Queries to this contract
///
/// End users query the contract state by directly sending Queries to the pRuntime without going on chain.
/// They should not change the contract state.
#[derive(Encode, Decode, Debug, Clone)]
pub enum Request {
    /// Query the current owner of the contract
    QueryOwner,
    /// Query the authentication token of telegram bot
    /// refer to: https://core.telegram.org/bots/api#authorizing-your-bot
    QueryBotToken,
    /// Query the identifier to target chat
    /// refer to: https://core.telegram.org/bots/api#sendmessage
    QueryChatId,
}

/// The Query results
#[derive(Encode, Decode, Debug, Clone)]
pub enum Response {
    Owner(AccountId),
    BotToken(String),
    ChatId(String),
}

#[derive(Encode, Decode, Debug)]
pub enum Error {
    OriginUnavailable,
    NotAuthorized,
}

impl BtcPriceBot {
    pub fn new() -> Self {
        BtcPriceBot {
            owner: Default::default(),
            bot_token: Default::default(),
            chat_id: Default::default(),
        }
    }
}

/// The payloads of the Telegram `sendMessage` request
/// refer to: https://core.telegram.org/bots/api#sendmessage
#[derive(Deserialize, Serialize)]
struct TgMessage {
    chat_id: String,
    text: String,
}

/// The BTC price from https://min-api.cryptocompare.com
#[derive(Deserialize, Serialize, Debug)]
struct BtcPrice {
    #[serde(rename(deserialize = "USD"))]
    usd: f64,
}

// Alice is the pre-defined root account in dev mode
const ALICE: &str = "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d";

impl contracts::NativeContract for BtcPriceBot {
    type Cmd = Command;
    type QReq = Request;
    type QResp = Result<Response, Error>;

    /// Return the contract id which uniquely identifies the contract
    fn id(&self) -> contracts::ContractId32 {
        contracts::BTC_PRICE_BOT
    }

    /// Handle the Commands from transactions on the blockchain. This method doesn't respond.
    fn handle_command(
        &mut self,
        context: &mut NativeContext,
        origin: MessageOrigin,
        cmd: Command,
    ) -> TransactionResult {
        info!("Command received: {:?}", &cmd);

        // we want to limit the sender who can use the Commands to the pre-define root account
        let sender = match &origin {
            MessageOrigin::AccountId(account) => AccountId::from(*account.as_fixed_bytes()),
            _ => return Err(TransactionError::BadOrigin),
        };
        let alice = contracts::account_id_from_hex(ALICE)
            .expect("should not failed with valid address; qed.");
        match cmd {
            Command::SetOwner { owner } => {
                if sender != alice {
                    return Err(TransactionError::BadOrigin);
                }
                self.owner = AccountId::from(*owner.as_fixed_bytes());
                Ok(())
            }
            Command::SetupBot { token, chat_id } => {
                if sender != alice && sender != self.owner {
                    return Err(TransactionError::BadOrigin);
                }
                self.bot_token = token;
                self.chat_id = chat_id;
                Ok(())
            }
            Command::ReportBtcPrice => {
                if sender != alice && sender != self.owner {
                    return Err(TransactionError::BadOrigin);
                }

                let bot_token = self.bot_token.clone();
                let chat_id = self.chat_id.clone();

                // This Command triggers the use of `AsyncSideTask`, it first send a HTTP request to get the current BTC
                // price from https://min-api.cryptocompare.com/, then sends the price to a Telegram bot through another
                // HTTP request
                //
                // To ensure the state consistency, the time to start the task and the time to upload the HTTP response
                // to chain must be determined. In this case, we start the task in the current `block_number`, and report
                // the result, whether succeeded or failed, to the chain after `duration`
                //
                // Report the result after 2 blocks no matter whether has received the HTTP response
                let block_number = context.block.block_number;
                let duration = 2;

                let task = AsyncSideTask::spawn(
                    block_number,
                    duration,
                    async move {
                        // Do network request in this block and return the result.
                        // Do NOT send mq message in this block.
                        log::info!("Side task starts to get BTC price");
                        let mut resp = match surf::get(
                            "https://min-api.cryptocompare.com/data/price?fsym=BTC&tsyms=USD",
                        )
                        .send()
                        .await
                        {
                            Ok(r) => r,
                            Err(err) => {
                                return format!("Network error: {:?}", err);
                            }
                        };
                        let result = match resp.body_string().await {
                            Ok(body) => body,
                            Err(err) => {
                                format!("Network error: {:?}", err)
                            }
                        };
                        log::info!("Side task got BTC price: {}", result);

                        let price: BtcPrice =
                            serde_json::from_str(result.as_str()).expect("broken BTC price result");
                        let text = format!("BTC price: ${}", price.usd);
                        let uri = format!(
                            "https://api.telegram.org/bot{}/{}",
                            bot_token, "sendMessage"
                        );
                        let data = &TgMessage { chat_id, text };

                        let mut resp = match surf::post(uri)
                            .body_json(data)
                            .expect("should not fail with valid data; qed.")
                            .await
                        {
                            Ok(r) => r,
                            Err(err) => {
                                return format!("Network error: {:?}", err);
                            }
                        };
                        let result = match resp.body_string().await {
                            Ok(body) => body,
                            Err(err) => {
                                format!("Network error: {:?}", err)
                            }
                        };
                        log::info!("Side task sent BTC price: {}", result);
                        result
                    },
                    |_result, _context| {
                        // You can send deterministic number of transactions in the result process
                        // In this case, we don't send the price since it has already been reported to the TG bot above
                    },
                );
                context.block.side_task_man.add_task(task);

                Ok(())
            }
        }
    }

    // Handle a direct Query and respond to it. It shouldn't modify the contract state.
    fn handle_query(
        &mut self,
        origin: Option<&chain::AccountId>,
        req: Request,
    ) -> Result<Response, Error> {
        info!("Query received: {:?}", &req);

        let sender = origin.ok_or(Error::OriginUnavailable)?;
        let alice = contracts::account_id_from_hex(ALICE)
            .expect("should not failed with valid address; qed.");
        match req {
            Request::QueryOwner => Ok(Response::Owner(self.owner.clone())),
            Request::QueryBotToken => {
                if sender != &alice && sender != &self.owner {
                    return Err(Error::NotAuthorized);
                }

                Ok(Response::BotToken(self.bot_token.clone()))
            }
            Request::QueryChatId => {
                if sender != &alice && sender != &self.owner {
                    return Err(Error::NotAuthorized);
                }

                Ok(Response::ChatId(self.chat_id.clone()))
            }
        }
    }
}
