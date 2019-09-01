/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::convert::From;

use serde::Serialize;

use crate::ccadb::{CCADBEntry, CCADBReport};
use crate::firefox::cert_storage::{CertStorage, IssuerSerial};
use crate::kinto::Kinto;
use crate::revocations_txt::*;

mod asn1;

//1. In Kinto but not in cert_storage
//2. In cert_storage but not in Kinto
//3. In cert_storage but not in revocations.txt
//4. In revocations.txt but not in cert_storage
//5. in revocations.txt but not in Kinto.
//6. In Kinto but not in revocations.txt

#[derive(Serialize)]
pub struct Return {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_kinto_not_in_cert_storage: Option<Vec<Intermediary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_cert_storage_not_in_kinto: Option<Vec<Intermediary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_cert_storage_not_in_revocations: Option<Vec<Intermediary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_revocations_not_in_cert_storage: Option<Vec<Intermediary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_revocations_not_in_kinto: Option<Vec<Intermediary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_kinto_not_in_revocations: Option<Vec<Intermediary>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_ccadb_not_in_cert_storage: Option<Vec<Intermediary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_cert_storage_not_in_ccadb: Option<Vec<Intermediary>>,
}

type WithRevocations = (CertStorage, Kinto, Revocations);
type WithoutRevocations = (CertStorage, Kinto);
type CCADBDiffCertStorage = (CertStorage, CCADBReport);

impl From<WithRevocations> for Return {
    fn from(values: WithRevocations) -> Self {
        let cert_storage: HashSet<Intermediary> = values.0.into();
        let kinto: HashSet<Intermediary> = values.1.into();
        let revocations: HashSet<Intermediary> = values.2.into();
        Return {
            in_kinto_not_in_cert_storage: Some(kinto
                .difference(&cert_storage)
                .cloned()
                .collect::<Vec<Intermediary>>()),
            in_cert_storage_not_in_kinto: Some(cert_storage
                .difference(&kinto)
                .cloned()
                .collect::<Vec<Intermediary>>()),
            in_cert_storage_not_in_revocations: Some(
                cert_storage
                    .difference(&revocations)
                    .cloned()
                    .collect::<Vec<Intermediary>>(),
            ),
            in_revocations_not_in_cert_storage: Some(
                revocations
                    .difference(&cert_storage)
                    .cloned()
                    .collect::<Vec<Intermediary>>(),
            ),
            in_revocations_not_in_kinto: Some(
                revocations
                    .difference(&kinto)
                    .cloned()
                    .collect::<Vec<Intermediary>>(),
            ),
            in_kinto_not_in_revocations: Some(
                kinto
                    .difference(&revocations)
                    .cloned()
                    .collect::<Vec<Intermediary>>(),
            ),
            in_ccadb_not_in_cert_storage: None,
            in_cert_storage_not_in_ccadb: None
        }
    }
}

impl From<WithoutRevocations> for Return {
    fn from(values: WithoutRevocations) -> Self {
        let cert_storage: HashSet<Intermediary> = values.0.into();
        let kinto: HashSet<Intermediary> = values.1.into();
        Return {
            in_kinto_not_in_cert_storage: Some(kinto
                .difference(&cert_storage)
                .cloned()
                .collect::<Vec<Intermediary>>()),
            in_cert_storage_not_in_kinto: Some(cert_storage
                .difference(&kinto)
                .cloned()
                .collect::<Vec<Intermediary>>()),
            in_cert_storage_not_in_revocations: None,
            in_revocations_not_in_cert_storage: None,
            in_revocations_not_in_kinto: None,
            in_kinto_not_in_revocations: None,
            in_ccadb_not_in_cert_storage: None,
            in_cert_storage_not_in_ccadb: None
        }
    }
}

impl From<CCADBDiffCertStorage> for Return {
    fn from(values : CCADBDiffCertStorage) -> Self {
        let cert_storage: HashSet<Intermediary> = values.0.into();
        let cccad_report: HashSet<Intermediary> = values.1.into();
        Return {
            in_kinto_not_in_cert_storage: None,
            in_cert_storage_not_in_kinto: None,
            in_cert_storage_not_in_revocations: None,
            in_revocations_not_in_cert_storage: None,
            in_revocations_not_in_kinto: None,
            in_kinto_not_in_revocations: None,
            in_ccadb_not_in_cert_storage: Some(cccad_report.difference(&cert_storage).cloned().collect()),
            in_cert_storage_not_in_ccadb: Some(cert_storage.difference(&cccad_report).cloned().collect())
        }
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Serialize, Clone)]
pub struct Intermediary {
    pub common_name: String,
    pub organization: String,
    pub serial: String,
}

impl From<Revocations> for HashSet<Intermediary> {
    /// Flattens out:
    ///     issuer
    ///      serial
    ///      serial
    ///      serial
    /// To:
    ///     [(issuer, serial), (issuer, serial), (issuer, serial)]
    fn from(revocations: Revocations) -> Self {
        let issuers = asn1::parse_issuers(
            revocations
                .data
                .iter()
                .map(|issuer| issuer.issuer_name.as_ref())
                .collect(),
        )
        .unwrap();
        let mut set: HashSet<Intermediary> = HashSet::new();
        for i in 0..issuers.len() {
            for serial in revocations.data.get(i).unwrap().serials.iter() {
                unsafe {
                    set.insert(Intermediary {
                        common_name: issuers.get_unchecked(i).common_name.clone(),
                        organization: issuers.get_unchecked(i).organization.clone(),
                        serial: serial.clone(),
                    });
                }
            }
        }
        set
    }
}

impl From<Kinto> for HashSet<Intermediary> {
    /// The interesting thing to point out here is that Kinto has
    /// many duplicate issuer/serial pairs for which I am not keen
    /// as to the purpose. They have different "id"s, which I reckon
    /// are Kinto specific IDs, however I am implicitly deduplicating
    /// Kinto in this regard by shoving everything into a set.
    ///
    /// Please see kinto:tests::find_duplicates
    fn from(kinto: Kinto) -> Self {
        let issuers = asn1::parse_issuers(
            kinto
                .data
                .iter()
                .map(|issuer| issuer.issuer_name.as_ref())
                .collect(),
        )
        .unwrap();
        let mut set: HashSet<Intermediary> = HashSet::new();
        for i in 0..issuers.len() {
            unsafe {
                set.insert(Intermediary {
                    common_name: issuers.get_unchecked(i).common_name.clone(),
                    organization: issuers.get_unchecked(i).organization.clone(),
                    serial: kinto.data.get(i).unwrap().serial_number.clone(),
                });
            }
        }
        set
    }
}

impl From<CertStorage> for HashSet<Intermediary> {
    fn from(cs: CertStorage) -> Self {
        let cs: Vec<IssuerSerial> = cs.data.iter().cloned().collect();
        let issuers = asn1::parse_issuers(
            cs.iter()
                .map(|issuer| issuer.issuer_name.as_ref())
                .collect(),
        )
        .unwrap();
        let mut set = HashSet::new();
        for i in 0..issuers.len() {
            unsafe {
                set.insert(Intermediary {
                    common_name: issuers.get_unchecked(i).common_name.clone(),
                    organization: issuers.get_unchecked(i).organization.clone(),
                    serial: cs.get(i).unwrap().serial.clone(),
                });
            }
        }
        set
    }
}

impl From<CCADBReport> for HashSet<Intermediary> {
    fn from(ccadb: CCADBReport) -> Self {
        ccadb
            .report
            .into_iter()
            .map(|entry| Intermediary {
                common_name: entry.certificate_issuer_common_name.clone(),
                organization: entry.certificate_issuer_organization,
                serial: base64::encode(
                    &hex::decode(entry.certificate_serial_number.as_bytes()).unwrap(),
                ),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::*;
    use reqwest::Url;
    use std::convert::TryInto;

    use crate::kinto::tests::*;
    use crate::revocations_txt::tests::*;

    #[test]
    fn smoke_from_revocations() -> Result<()> {
        let rev: Revocations = REVOCATIONS_TXT
            .parse::<Url>()
            .chain_err(|| "bad URL")?
            .try_into()?;
        let int: HashSet<Intermediary> = rev.into();
        eprintln!("int = {:#?}", int);
        Ok(())
    }

    #[test]
    fn smoke_from_kinto() -> Result<()> {
        let kinto: Kinto = KINTO.parse::<Url>().chain_err(|| "bad URL")?.try_into()?;
        let int: HashSet<Intermediary> = kinto.into();
        eprintln!("int = {:#?}", int);
        Ok(())
    }
}
