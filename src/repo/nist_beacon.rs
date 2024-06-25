use bitvec::{order::Msb0, slice::BitSlice, vec::BitVec, view::BitView};
use entity::{prelude::*, *};
use poise::serenity_prelude::futures::{stream, StreamExt};
use sea_orm::{ActiveValue, DatabaseConnection, EntityTrait, QueryOrder};
use serde::Deserialize;
use serde_hex::{SerHex, StrictCap};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

const URL: &'static str = "https://beacon.nist.gov/beacon/2.0/pulse/last";
const N_BYTES: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NistBeaconResponse {
    pub pulse: NistBeaconPulse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NistBeaconPulse {
    pub uri: String,
    pub chain_index: i32,
    pub pulse_index: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub time_stamp: OffsetDateTime,
    #[serde(with = "SerHex::<StrictCap>")]
    pub output_value: [u8; N_BYTES],
}

#[derive(Debug, thiserror::Error)]
pub enum NistBeaconRepoErr {
    #[error("NistBeaconRepoErr/RewestErr: {0}")]
    ReqwestErr(#[from] reqwest::Error),
    #[error("NistBeaconRepoErr/DbErr: {0}")]
    DbErr(#[from] sea_orm::DbErr),
    #[error("NistBeaconRepoErr/TimeFmtErr: {0}")]
    TimeFmtErr(#[from] time::error::Format),
    #[error("NistBeaconRepoErr/NoNewRand: {0}")]
    NoNewRand(String),
}

pub type Result<T, E = NistBeaconRepoErr> = std::result::Result<T, E>;

pub struct NistBeaconRepo {
    url: String,
    db: DatabaseConnection,
    bitq: tokio::sync::Mutex<BitQueue>,
}

impl NistBeaconRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            url: URL.into(),
            db,
            bitq: BitQueue::new().into(),
        }
    }

    #[inline]
    pub async fn get_nist_current_rand(url: &str) -> Result<NistBeaconPulse> {
        let res = reqwest::get(url)
            .await?
            .json::<NistBeaconResponse>()
            .await?;

        Ok(res.pulse)
    }

    pub fn get_new_rand_stream<'a>(
        nist_url: &'a str,
        db: DatabaseConnection,
    ) -> impl StreamExt<Item = Result<[u8; N_BYTES]>> + 'a {
        stream::unfold((nist_url, db), |(nist_url, db)| async move {
            macro_rules! shoot {
                ($x:expr) => {
                    match $x {
                        Ok(o) => o,
                        Err(e) => return Some((Err(e.into()), (nist_url, db))),
                    }
                };
            }
            let curr = shoot! {
                Self::get_nist_current_rand(nist_url).await
            };
            let stored = shoot! {
                NistRandEntry::find()
                    .order_by_desc(nist_rand_entry::Column::ChainIndex)
                    .order_by_desc(nist_rand_entry::Column::PulseIndex)
                    .one(&db)
                    .await
            };
            if let Some(s) = stored {
                if curr.chain_index == s.chain_index && curr.pulse_index == s.pulse_index {
                    return Some((
                        Err(NistBeaconRepoErr::NoNewRand(format!(
                            "({},{}) already existed in database",
                            s.chain_index, s.pulse_index
                        ))),
                        (nist_url, db),
                    ));
                }
            }

            let new_store = nist_rand_entry::ActiveModel {
                chain_index: ActiveValue::set(curr.chain_index),
                pulse_index: ActiveValue::set(curr.pulse_index),
                timestamp: ActiveValue::set(shoot!(curr
                    .time_stamp
                    .to_offset(UtcOffset::UTC)
                    .format(&Rfc3339))),
                uri: ActiveValue::set(curr.uri),
                output_value: ActiveValue::set(curr.output_value.to_vec()),
            };
            _ = shoot! {
                NistRandEntry::insert(new_store).exec(&db).await
            };

            Some((Ok(curr.output_value), (nist_url, db)))
        })
    }

    pub async fn rand(&self, from: i64, to: i64) -> Result<i64> {
        Self::_rand(
            &self.bitq,
            from,
            to,
            Self::get_new_rand_stream(&self.url, self.db.clone()),
        )
        .await
    }

    async fn _rand(
        bitq: &tokio::sync::Mutex<BitQueue>,
        from: i64,
        to: i64,
        new_rand: impl StreamExt<Item = Result<[u8; N_BYTES]>>,
    ) -> Result<i64> {
        tokio::pin!(new_rand);

        let n = to - from + 1;
        if n <= 1 {
            return Ok(from);
        }
        let n = n as u32;

        let k = {
            let y = n.ilog2();

            // Check if n is power of 2
            if n & (n - 1) == 0 {
                y
            } else {
                y + 1
            }
        };

        macro_rules! pop {
            ($a:expr) => {{
                let mut bitq = bitq.lock().await;
                if $a > bitq.len() {
                    let new = new_rand
                        .next()
                        .await
                        .unwrap_or(Err(NistBeaconRepoErr::NoNewRand("stream ended".into())))?;
                    bitq.insert_new_rand(&new);
                }
                bitq.pop_front($a)
            }};
        }

        let mut rand_bits = pop!(k as usize);
        loop {
            // dbg!(&rand_bits);
            let num = {
                let mut dst = [0u8; 4];
                for (i, bit) in rand_bits.iter().rev().enumerate() {
                    let dst_i = 3 - i / 8;
                    let bitmask = if *bit { 1u8 << i % 8 } else { 0 };
                    // dbg!(i, *bit, dst_i, bitmask);
                    dst[dst_i] |= bitmask;
                }

                // dbg!(dst);

                u32::from_be_bytes(dst)
            };
            // dbg!(num, n);

            if num < n {
                return Ok(from + (num as i64));
            }

            rand_bits = pop!(k as usize);
            // dbg!(&rand_bits);
        }
    }
}

#[derive(Debug)]
struct BitQueue {
    arr: [u8; N_BYTES * 2],
    i: usize,
    n: usize,
    start_at_zero: bool,
}
impl BitQueue {
    #[inline]
    fn new() -> Self {
        Self {
            arr: [0u8; N_BYTES * 2],
            i: 0,
            n: 0,
            start_at_zero: true,
        }
    }

    #[inline]
    fn bits(&self) -> &BitSlice<u8, Msb0> {
        self.arr.view_bits::<Msb0>()
    }

    #[inline]
    fn len(&self) -> usize {
        self.n
    }

    fn insert_new_rand(&mut self, new_rand: &[u8; N_BYTES]) {
        let data_i_start = if self.start_at_zero { 0usize } else { N_BYTES };

        for i in 0..N_BYTES {
            let di = data_i_start + i;
            self.arr[di] = new_rand[i];
        }

        self.start_at_zero = !self.start_at_zero;
        self.n += N_BYTES * 8;

        // dbg!(self.arr);
    }

    fn pop_front(&mut self, n: usize) -> BitVec<u8, Msb0> {
        assert!(n <= self.len());

        let bits = self.bits();
        let s1 = self.i;
        let t = s1 + n;
        let (e1, s2, e2, new_i) = if t > bits.len() {
            (bits.len(), 0, t - bits.len(), t - bits.len())
        } else {
            (t, 0, 0, (t % bits.len()))
        };

        let mut res = BitVec::from_bitslice(&bits[s1..e1]);
        if e2 > s2 {
            res.extend_from_bitslice(&bits[s2..e2]);
        }

        self.i = new_i;
        self.n -= n;
        res
    }
}

#[cfg(test)]
mod tests {
    use super::{BitQueue, N_BYTES};

    #[test]
    fn test_bitqueue_simple() {
        let bq = BitQueue::new();
        assert_eq!(bq.arr, [0u8; N_BYTES * 2]);
        assert_eq!(bq.i, 0);
        assert_eq!(bq.len(), 0);
        assert!(bq.start_at_zero);
    }

    #[test]
    fn test_bitqueue_insert_one() {
        let mut bq = BitQueue::new();
        let mut new_rand = [0u8; N_BYTES];
        const S: u8 = 0b1100_0000;
        const E: u8 = 0b0000_0101;
        new_rand[0] = S;
        new_rand[N_BYTES - 1] = E;

        bq.insert_new_rand(&new_rand);

        let mut exp_rand = [0u8; N_BYTES * 2];
        exp_rand[0] = S;
        exp_rand[N_BYTES - 1] = E;
        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, 0);
        assert_eq!(bq.len(), N_BYTES * 8);
        assert!(!bq.start_at_zero);

        let e = bq.pop_front(3);
        assert_eq!(e.len(), 3);
        assert!(e[0]);
        assert!(e[1]);
        assert!(!e[2]);

        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, 3);
        assert_eq!(bq.len(), N_BYTES * 8 - 3);
        assert!(!bq.start_at_zero);

        _ = bq.pop_front(5 + (N_BYTES - 2) * 8 + 3);

        let e = bq.pop_front(5);
        assert_eq!(e.len(), 5);
        assert!(!e[0]);
        assert!(!e[1]);
        assert!(e[2]);
        assert!(!e[3]);
        assert!(e[4]);

        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, N_BYTES * 8);
        assert_eq!(bq.len(), 0);
        assert!(!bq.start_at_zero);
    }

    #[test]
    fn test_bitqueue_insert_two() {
        let mut bq = BitQueue::new();
        let mut new_rand = [0u8; N_BYTES];
        const S: u8 = 0b1100_0000;
        const E: u8 = 0b0000_0101;
        new_rand[0] = S;
        new_rand[N_BYTES - 1] = E;

        bq.insert_new_rand(&new_rand);

        _ = bq.pop_front((N_BYTES - 1) * 8 + 3);

        // remain 5 insert another

        let mut exp_rand = [0u8; N_BYTES * 2];
        exp_rand[0] = S;
        exp_rand[N_BYTES - 1] = E;
        exp_rand[N_BYTES] = S;
        exp_rand[N_BYTES * 2 - 1] = E;

        bq.insert_new_rand(&new_rand);
        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, N_BYTES * 8 - 5);
        assert_eq!(bq.len(), N_BYTES * 8 + 5);
        assert!(bq.start_at_zero);

        let e = bq.pop_front(6);
        assert_eq!(e.len(), 6);
        assert!(!e[0]);
        assert!(!e[1]);
        assert!(e[2]);
        assert!(!e[3]);
        assert!(e[4]);
        assert!(e[5]);

        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, N_BYTES * 8 + 1);
        assert_eq!(bq.len(), N_BYTES * 8 - 1);
        assert!(bq.start_at_zero);

        _ = bq.pop_front(7 + (N_BYTES - 2) * 8 + 4);

        let e = bq.pop_front(2);
        assert_eq!(e.len(), 2);
        assert!(!e[0]);
        assert!(e[1]);

        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, N_BYTES * 8 * 2 - 2);
        assert_eq!(bq.len(), 2);
        assert!(bq.start_at_zero);

        let e = bq.pop_front(2);
        assert_eq!(e.len(), 2);
        assert!(!e[0]);
        assert!(e[1]);

        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, 0);
        assert_eq!(bq.len(), 0);
        assert!(bq.start_at_zero);
    }

    #[test]
    fn test_bitqueue_insert_three() {
        let mut bq = BitQueue::new();
        let mut new_rand = [0u8; N_BYTES];
        const S: u8 = 0b1100_0000;
        const E: u8 = 0b0000_0101;
        new_rand[0] = S;
        new_rand[N_BYTES - 1] = E;

        bq.insert_new_rand(&new_rand);
        bq.insert_new_rand(&new_rand);

        _ = bq.pop_front(N_BYTES * 2 * 8 - 3);

        assert_eq!(bq.len(), 3);

        const NS: u8 = 0b0100_0000;
        const NE: u8 = 0b0000_0010;
        new_rand[0] = NS;
        new_rand[N_BYTES - 1] = NE;

        let mut exp_rand = [0u8; N_BYTES * 2];
        exp_rand[0] = NS;
        exp_rand[N_BYTES - 1] = NE;
        exp_rand[N_BYTES] = S;
        exp_rand[N_BYTES * 2 - 1] = E;

        bq.insert_new_rand(&new_rand);
        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, N_BYTES * 2 * 8 - 3);
        assert_eq!(bq.len(), N_BYTES * 8 + 3);
        assert!(!bq.start_at_zero);

        let e = bq.pop_front(7);
        assert_eq!(e.len(), 7);
        assert!(e[0]);
        assert!(!e[1]);
        assert!(e[2]);
        assert!(!e[3]);
        assert!(e[4]);
        assert!(!e[5]);
        assert!(!e[6]);

        assert_eq!(bq.arr, exp_rand);
        assert_eq!(bq.i, 4);
        assert_eq!(bq.len(), N_BYTES * 8 - 4);
        assert!(!bq.start_at_zero);
    }
}
