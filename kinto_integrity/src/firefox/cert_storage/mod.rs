/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// The core logic of this module was shamelessly plucked from
// https://github.com/mozkeeler/cert-storage-inspector

use crate::errors::*;
use rkv::backend::{BackendEnvironmentBuilder, SafeMode};
use rkv::{Rkv, StoreOptions, Value};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::PathBuf;

mod new;

pub struct CertStorage {
    pub data: HashSet<IssuerSerial>,
}

#[derive(Eq, PartialEq, Hash)]
pub struct IssuerSerial {
    pub issuer_name: String,
    pub serial: String,
}

impl TryFrom<PathBuf> for CertStorage {
    type Error = Error;

    fn try_from(db_path: PathBuf) -> Result<Self> {
        let mut revocations = CertStorage {
            data: HashSet::new(),
        };
        let mut builder = Rkv::environment_builder::<SafeMode>();
        builder.set_max_dbs(2);
        builder.set_map_size(16777216);
        let env = match Rkv::from_builder(&db_path, builder) {
            Err(err) => Err(format!("{}", err))?,
            Ok(env) => env,
        };
        let store = env.open_single("cert_storage", StoreOptions::default())?;
        let reader = env.read()?;
        for item in store.iter_start(&reader)? {
            let (key, value) = item?;
            let is = match key {
                [b'i', b's', entry @ ..] => decode_revocation(entry, &value),
                [b's', b'p', b'k', entry @ ..] => decode_revocation(entry, &value),
                _ => None
            };
            match is {
                Some(Ok(issuer_serial)) => {
                    revocations.data.insert(issuer_serial);
                }
                Some(Err(err)) => {
                    Err(err).chain_err(|| "failed to build set from cert_storage")?;
                }
                None => {
                    ();
                }
            };
        }
        Ok(revocations)
    }
}

pub enum Entry {
    IssuerSerial {
        issuer_name: String,
        serial: String
    },
    SubjectKeyHash {
        subject: String,
        key_hash: String
    }
}

impl Entry {
    fn issuer_serial_from(parts: (&[u8], &[u8])) -> Entry {
        Entry::IssuerSerial {
            issuer_name: base64::encode(parts.0),
            serial: base64::encode(parts.1)
        }
    }

    fn subject_key_hash_from(parts: (&[u8], &[u8])) -> Entry {
        Entry::SubjectKeyHash {
            subject: base64::encode(parts.0),
            key_hash: base64::encode(parts.1)
        }
    }
}

fn decode(key: &[u8], value: Option<Value>) -> Result<Option<Entry>> {
    match value {
        Some(Value::I64(1)) => (),
        Some(Value::I64(0)) => return Ok(None),
        None => return Ok(None),
        Some(_) => return Ok(None),
    };
    Ok(match key {
        [b'i', b's', entry @ ..] => Some(Entry::issuer_serial_from(split_der_key(entry)?)),
        [b's', b'p', b'k', entry @ ..] => Some(Entry::subject_key_hash_from(split_der_key(entry)?)),
        _ => None
    })
}

fn decode_revocation(key: &[u8], value: &Option<Value>) -> Option<Result<IssuerSerial>> {
    match *value {
        Some(Value::I64(i)) if i == 1 => {}
        Some(Value::I64(i)) if i == 0 => return None,
        None => return None,
        Some(_) => return None,
    }
    Some(match split_der_key(key) {
        Ok((part1, part2)) => Ok(IssuerSerial {
            issuer_name: base64::encode(part1),
            serial: base64::encode(part2),
        }),
        Err(e) => Err(e),
    })
}

fn split_der_key(key: &[u8]) -> Result<(&[u8], &[u8])> {
    if key.len() < 2 {
        return Err("key too short to be DER".into());
    }
    let first_len_byte = key[1] as usize;
    if first_len_byte < 0x80 {
        if key.len() < first_len_byte + 2 {
            return Err("key too short".into());
        }
        return Ok(key.split_at(first_len_byte + 2 as usize));
    }
    if first_len_byte == 0x80 {
        return Err("unsupported ASN.1".into());
    }
    if first_len_byte == 0x81 {
        if key.len() < 3 {
            return Err("key too short to be DER".into());
        }
        let len = key[2] as usize;
        if len < 0x80 {
            return Err("bad DER".into());
        }
        if key.len() < len + 3 {
            return Err("key too short".into());
        }
        return Ok(key.split_at(len + 3));
    }
    if first_len_byte == 0x82 {
        if key.len() < 4 {
            return Err("key too short to be DER".into());
        }
        let len = (key[2] as usize) << 8 | key[3] as usize;
        if len < 256 {
            return Err("bad DER".into());
        }
        if key.len() < len + 4 {
            return Err("key too short".into());
        }
        return Ok(key.split_at(len + 4));
    }
    Err("key too long".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn asdasd() {
        let c: CertStorage = PathBuf::from("/home/chris/security_state").try_into().unwrap();
        for e in c.data {
            println!("{}", e.issuer_name)
        }
    }
}