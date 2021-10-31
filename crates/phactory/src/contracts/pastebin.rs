use anyhow::Result;
use log::info;
use parity_scale_codec::{Decode, Encode};
use phala_mq::MessageOrigin;
use sp_core::hashing;
use std::convert::TryInto;

use super::{TransactionError, TransactionResult};
use crate::contracts;
use crate::contracts::{AccountId, NativeContext};
extern crate runtime as chain;

use phala_types::messaging::PastebinCommand;

/// Contract Overview
///
/// The contracts of Phala Network will handle two kinds of requests: Command and Query.
/// (we name Query as `Request` in the following code)
///
/// The Commands are allowed to update the state of contract. They are first sent to the blockchain, and then distributed
/// the according contract. Such design ensures the state consistency across multiple instances of the same contract, since
/// all the instances will reach the same state after replaying all the Commands.
/// Such property limits the use of random generator in our contracts: you can only generate random with on-chain entropy,
/// because off-chain random generation can break the state consistency. We will show an example in the following code.
///
/// The Queries are not allowed to change the state of contract. They are directly sent to contract through the local rpc
/// endpoint. Since they are off-chain requests, they can be sent and then real-time processed.
///
/// For the advanced usage of HTTP request in contract, refer to `btc_price_bot.rs`.

/// The Commands to this contract
///
/// Commands need to be first posted on chain then will be dispatched to the contract, that why we define the `PastebinCommand`
/// in phala_types to be used globally.
/// They can change the state of the contract, with no responses.
type Command = PastebinCommand;

type PostId = String;

type PostContent = String;

type CreateOn = u64;

fn now() -> u64 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    now.as_secs()
}

// Post state for each bin
#[derive(Encode, Decode, Debug, Clone, Default)]
pub struct Post {
    id: PostId,
    content: PostContent,
    owner: AccountId,
    /// TODO: change this with array
    readable_by: AccountId,
    created_on: CreateOn,
}

// impl codec::WrapperTypeEncode for Post {}

/// Contract state
#[derive(Encode, Decode, Debug, Clone, Default)]
pub struct Pastebin {
    /// TODO: change this with Vector and add index
    post: Post,
}

/// The Queries to this contract
///
/// End users query the contract state by directly sending Queries to the pRuntime without going on chain.
/// They should not change the contract state.
#[derive(Encode, Decode, Debug, Clone)]
pub enum Request {
    /// Query the content of pastebin
    QueryPost,
}

/// The Query results
#[derive(Encode, Decode, Debug, Clone)]
pub enum Response {
    Post(Post),
}

#[derive(Encode, Decode, Debug)]
pub enum Error {
    OriginUnavailable,
    NotAuthorized,
}

impl Pastebin {
    pub fn new() -> Self {
        Pastebin {
            post: Default::default(),
        }
    }
}

// Alice is the pre-defined root account in dev mode
const ALICE: &str = "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d";

impl contracts::NativeContract for Pastebin {
    type Cmd = Command;
    type QReq = Request;
    type QResp = Result<Response, Error>;

    /// Return the contract id which uniquely identifies the contract
    fn id(&self) -> contracts::ContractId32 {
        contracts::PASTEBIN
    }

    /// Handle the Commands from transactions on the blockchain. This method doesn't respond.
    ///
    /// # Arguments
    ///
    /// * `context` - The current block info with the necessary egress channel
    /// * `origin` - The sender of the Command, can be Pallet, pRuntime, Contract, Account or even entities from other chain
    /// * `cmd` - The on-chain Command to process
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
            Command::CreatePost {
                id,
                owner,
                readable_by,
                content,
            } => {
                println!(
                    "owner: {}, readable_by: {}, content: {}",
                    owner, readable_by, content
                );
                if sender != alice {
                    return Err(TransactionError::BadOrigin);
                }
                // let my_uuid = Uuid::new_v4()?;
                //     println!("{}", my_uuid.to_simple().to_string());
                let post = Post {
                    id: id,
                    owner: AccountId::from(*owner.as_fixed_bytes()),
                    readable_by: AccountId::from(*readable_by.as_fixed_bytes()),
                    content: content,
                    created_on: now(),
                };
                self.post = post;
                Ok(())
            }
        }
    }

    /// Handle a direct Query and respond to it. It shouldn't modify the contract state.
    ///
    /// # Arguments
    ///
    /// * `origin` - For off-chain Query, the sender can only be AccountId
    /// * `req` — Off-chain Query to handle
    fn handle_query(
        &mut self,
        origin: Option<&chain::AccountId>,
        req: Request,
    ) -> Result<Response, Error> {
        info!("Query received: {:?}", &req);
        match req {
            Request::QueryPost => {
                // also, we only allow Alice or contract owner to peek the number
                let sender = origin.ok_or(Error::OriginUnavailable)?;
                let alice = contracts::account_id_from_hex(ALICE)
                    .expect("should not failed with valid address; qed.");

                if sender != &alice && sender != &self.post.readable_by {
                    return Err(Error::NotAuthorized);
                }

                let post = self.post.clone();

                Ok(Response::Post(post))
            }
        }
    }
}
