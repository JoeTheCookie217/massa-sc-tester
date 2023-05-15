use crate::execution_context::{AsyncMessage, ExecutionContext, Slot};

use anyhow::{bail, Result};
use json::object;
use massa_hash::Hash;
use massa_sc_runtime::{Interface, InterfaceClone};
use rand::RngCore;

impl InterfaceClone for ExecutionContext {
    fn clone_box(&self) -> Box<dyn Interface> {
        Box::new(self.clone())
    }
}

impl Interface for ExecutionContext {
    fn print(&self, message: &str) -> Result<()> {
        let json = object!(
            print: {
                message: message
            }
        );
        self.update_execution_trace(json)?;
        Ok(())
    }

    fn init_call(&self, address: &str, raw_coins: u64) -> Result<Vec<u8>> {
        let entry = self.get_entry(address)?;
        let from_address = self.call_stack_peek()?.address;
        if raw_coins > 0 {
            self.transfer_coins_for(&from_address, address, raw_coins)?
        }
        self.call_stack_push(crate::execution_context::CallItem {
            address: address.to_owned(),
            coins: raw_coins,
        })?;
        Ok(entry.get_bytecode())
    }

    /// Returns zero as a default if address not found.
    fn get_balance(&self) -> Result<u64> {
        let address = &self.call_stack_peek()?.address;
        let balance = self.get_entry(address)?.balance;
        let json = object!(
            get_balance: {
                return_value: balance
            }
        );
        self.update_execution_trace(json)?;
        Ok(balance)
    }

    /// Returns zero as a default if address not found.
    fn get_balance_for(&self, address: &str) -> Result<u64> {
        let balance = self.get_entry(address)?.balance;
        let json = object!(
            get_balance_for: {
                address: address,
                return_value: balance
            }
        );
        self.update_execution_trace(json)?;
        Ok(balance)
    }

    /// Pops the last element of the call stack
    fn finish_call(&self) -> Result<()> {
        self.call_stack_pop()
    }

    /// Creates a new address that contains the sent bytecode
    fn create_module(&self, module: &[u8]) -> Result<String> {
        let mut rbytes = [0; 128];
        rand::thread_rng().fill_bytes(&mut rbytes);
        let hash = Hash::compute_from(&rbytes);
        let address = hash.to_bs58_check();

        self.set_module(&address, module)?;
        self.own_insert(&address)?;
        let json = object!(
            create_module: {
                module: module,
                return_value: address.clone()
            }
        );
        self.update_execution_trace(json)?;
        Ok(address)
    }

    /// Requires the data at the address
    fn raw_get_data_for(&self, address: &str, key: &[u8]) -> Result<Vec<u8>> {
        let data = self.get(address)?.get_data(key);
        let json = object!(
            raw_get_data_for: {
                address: address,
                key: key,
                return_value: data.clone(),
            }
        );
        self.update_execution_trace(json)?;
        Ok(data)
    }

    /// Requires to replace the data in the current address
    ///
    /// Note:
    /// The execution lib will allways use the current context address for the update
    fn raw_set_data_for(&self, address: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let curr_address = self.call_stack_peek()?.address;
        let json = object!(
            raw_set_data_for: {
                address: address,
                key: key,
                value: value,
            }
        );
        self.update_execution_trace(json)?;
        if self.own(address)? || *address == curr_address {
            self.set_data_entry(address, key, value)?;
            Ok(())
        } else {
            bail!("you do not have write access to this entry")
        }
    }

    fn raw_get_data(&self, key: &[u8]) -> Result<Vec<u8>> {
        let data = self.get(&self.call_stack_peek()?.address)?.get_data(key);
        let json = object!(
            raw_get_data: {
                key: key,
                return_value: data.clone()
            }
        );
        self.update_execution_trace(json)?;
        Ok(data)
    }

    fn raw_set_data(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let json = object!(
            raw_set_data: {
                key: key,
                value: value
            }
        );
        self.update_execution_trace(json)?;
        self.set_data_entry(&self.call_stack_peek()?.address, key, value)
    }

    /// Transfer coins from the current address to a target address
    /// to_address: target address
    /// raw_amount: amount to transfer (in raw u64)
    fn transfer_coins(&self, to_address: &str, raw_amount: u64) -> Result<()> {
        let json = object!(
            transfer_coins: {
                to_address: to_address,
                raw_amount: raw_amount
            }
        );
        self.update_execution_trace(json)?;
        let from_address = self.call_stack_peek()?.address;
        self.transfer_coins_for(&from_address, to_address, raw_amount)
    }

    /// Transfer coins from the current address to a target address
    /// from_address: source address
    /// to_address: target address
    /// raw_amount: amount to transfer (in raw u64)
    fn transfer_coins_for(
        &self,
        from_address: &str,
        to_address: &str,
        raw_amount: u64,
    ) -> Result<()> {
        // debit
        self.sub(from_address, raw_amount)?;
        // credit
        if let Err(err) = self.add(to_address, raw_amount) {
            // cancel debit
            self.add(from_address, raw_amount)
                .expect("credit failed after same-amount debit succeeded");
            bail!("error crediting destination balance: {}", err);
        }
        let json = object!(
            transfer_coins_for: {
                from_address: from_address,
                to_address: to_address,
                raw_amount: raw_amount
            }
        );
        self.update_execution_trace(json)?;
        Ok(())
    }

    /// Return the list of owned adresses of a given SC user
    fn get_owned_addresses(&self) -> Result<Vec<String>> {
        let owned = self.owned_to_vec()?;
        let json = object!(
            get_owned_addresses: {
                return_value: owned.clone()
            }
        );
        self.update_execution_trace(json)?;
        Ok(owned)
    }

    fn get_call_stack(&self) -> Result<Vec<String>> {
        let callstack = self.callstack_to_vec()?;
        let json = object!(
            get_call_stack: {
                return_value: callstack.clone()
            }
        );
        self.update_execution_trace(json)?;
        Ok(callstack)
    }

    fn generate_event(&self, data: String) -> Result<()> {
        let sender = self.call_stack_peek()?.address;
        self.push_event(self.execution_slot, sender, data.clone())?;
        let json = object!(
            generate_event: {
                return_value: data
            }
        );
        self.update_execution_trace(json)?;
        Ok(())
    }

    fn get_call_coins(&self) -> Result<u64> {
        let coins = self.call_stack_peek()?.coins;
        let json = object!(
            get_call_coins: {
                return_value: coins
            }
        );
        self.update_execution_trace(json)?;
        Ok(coins)
    }

    fn has_data(&self, key: &[u8]) -> Result<bool> {
        let ret_bool = self.get(&self.call_stack_peek()?.address)?.has_data(key);
        let json = object!(
            has_data: {
                key: key,
                return_value: ret_bool
            }
        );
        self.update_execution_trace(json)?;
        Ok(ret_bool)
    }

    fn hash(&self, key: &[u8]) -> Result<[u8; 32]> {
        let mut rbytes = [0; 128];
        rand::thread_rng().fill_bytes(&mut rbytes);
        let hash = Hash::compute_from(&rbytes);

        let json = object!(
            hash: {
                key: key,
                return_value: hash.to_bs58_check()
            }
        );
        self.update_execution_trace(json)?;
        Ok(hash.into_bytes())
    }

    fn raw_set_bytecode_for(&self, address: &str, bytecode: &[u8]) -> Result<()> {
        self.set_module(address, bytecode)?;
        let json = object!(
            raw_set_bytecode_for: {
                address: address,
                return_value: bytecode
            }
        );
        self.update_execution_trace(json)?;
        Ok(())
    }

    fn raw_set_bytecode(&self, bytecode: &[u8]) -> Result<()> {
        self.set_module(&self.call_stack_peek()?.address, bytecode)?;
        let json = object!(
            raw_set_bytecode: {
                return_value: bytecode
            }
        );
        self.update_execution_trace(json)?;
        Ok(())
    }

    fn unsafe_random(&self) -> Result<i64> {
        let rnbr: i64 = rand::random();
        let json = object!(
            unsafe_random: {
                return_value: rnbr
            }
        );
        self.update_execution_trace(json)?;
        Ok(rnbr)
    }

    fn get_current_period(&self) -> Result<u64> {
        let json = object!(
            get_current_period: {
                return_value:  self.execution_slot.period
            }
        );
        self.update_execution_trace(json)?;
        Ok(self.execution_slot.period)
    }

    fn get_current_thread(&self) -> Result<u8> {
        let json = object!(
            get_current_thread: {
                return_value:  self.execution_slot.thread
            }
        );
        self.update_execution_trace(json)?;
        Ok(self.execution_slot.thread)
    }

    fn send_message(
        &self,
        target_address: &str,
        target_handler: &str,
        validity_start: (u64, u8),
        validity_end: (u64, u8),
        max_gas: u64,
        gas_price: u64,
        coins: u64,
        data: &[u8],
        _filter: Option<(&str, Option<&[u8]>)>,
    ) -> Result<()> {
        let sender = self.call_stack_peek()?.address;
        self.push_async_message(
            Slot {
                period: validity_start.0,
                thread: validity_start.1,
            },
            AsyncMessage {
                sender_address: sender.clone(),
                target_address: target_address.to_string(),
                target_handler: target_handler.to_string(),
                gas: max_gas,
                coins,
                data: data.to_vec(),
            },
        )?;
        let json = object!(
            send_message: {
                sender_address: sender,
                target_address: target_address,
                target_handler: target_handler,
                validity_start: {
                    period: validity_start.0,
                    thread: validity_start.1
                },
                validity_end: {
                    period: validity_end.0,
                    thread: validity_end.1
                },
                max_gas: max_gas,
                gas_price: gas_price,
                coins: coins,
                data: data,
            }
        );
        self.update_execution_trace(json)?;
        Ok(())
    }

    fn caller_has_write_access(&self) -> Result<bool> {
        let call_stack = self.get_call_stack()?;
        let mut call_stack_iter = call_stack.iter().rev();
        let caller_owned_addresses = if let Some(last) = call_stack_iter.next() {
            if let Some(prev_to_last) = call_stack_iter.next() {
                prev_to_last.clone()
            } else {
                last.clone()
            }
        } else {
            bail!("not found")
        };
        let current_address = self.call_stack_peek()?.address;
        Ok(caller_owned_addresses.contains(&current_address))
    }
}
