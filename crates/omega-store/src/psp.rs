//! PSP games: what a game's disc says it is.
//!
//! A UMD's image is an ISO 9660 disc of 2048-byte sectors, and its `PSP_GAME/PARAM.SFO` names the
//! game: `DISC_ID`, such as `ULUS10041`, and `TITLE`. An `.iso` is read sector by sector, a
//! `.cso` (CISO) the same way through its compressed blocks, and a `.pbp` (an eboot) from the
//! PARAM.SFO at its start. A `.chd` is compressed another way, and is not read.
//!
//! Only the handful of sectors that lead to PARAM.SFO are read, never the whole image.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::ps1::{find_entry, sfo_text, USER};

/// How many bytes a UMD's sector holds.
pub const SECTOR: usize = USER;

/// What a PSP game's PARAM.SFO says it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sfo {
    /// The disc's ID, as the disc prints it without its dash: `ULUS10041`.
    pub id: String,
    pub title: String,
}

/// The region a PSP game's ID says, by its third and fourth letters: `ULUS10041` is the USA's,
/// `ULES` Europe's, `ULJS` and `ULJM` Japan's; Asia's and Korea's are another.
pub fn id_region(id: &str) -> Option<crate::Region> {
    match id.get(2..4)?.to_ascii_uppercase().as_str() {
        "US" | "UH" => Some(crate::Region::Usa),
        "ES" | "EH" => Some(crate::Region::Europe),
        "JS" | "JM" | "JH" | "JG" | "JP" => Some(crate::Region::Japan),
        "AS" | "AH" | "KS" | "KH" => Some(crate::Region::Other),
        _ => None,
    }
}

/// What the PSP game at `path` says it is, where it can be read.
pub fn info(path: &Path) -> Option<Sfo> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let sfo = match ext.as_str() {
        "iso" => {
            let mut file = File::open(path).ok()?;
            disc_sfo(&mut |lba| iso_sector(&mut file, lba))
        }
        "cso" => {
            let mut cso = Cso::open(path)?;
            disc_sfo(&mut |lba| cso.sector(lba))
        }
        "pbp" => pbp_sfo(path),
        _ => None,
    }?;
    from_sfo(&sfo)
}

/// The ID and title a PARAM.SFO holds.
pub fn from_sfo(sfo: &[u8]) -> Option<Sfo> {
    let id = sfo_text(sfo, "DISC_ID")?.trim().to_ascii_uppercase();
    if id.is_empty() {
        return None;
    }
    Some(Sfo {
        id,
        title: sfo_text(sfo, "TITLE")
            .unwrap_or_default()
            .trim()
            .to_string(),
    })
}

/// The title a PSP save's PARAM.SFO gives its game: a save's has no `DISC_ID`, only `TITLE`.
pub fn save_title(sfo: &[u8]) -> Option<String> {
    let title = sfo_text(sfo, "TITLE")?.trim().to_string();
    (!title.is_empty()).then_some(title)
}

/// The game ID a PSP save's folder starts with: `ULUS10391DATA00` is `ULUS10391`'s.
pub fn save_id(folder: &str) -> Option<String> {
    let id = folder.get(..9)?;
    let (letters, digits) = id.split_at(4);
    (letters.chars().all(|c| c.is_ascii_alphabetic()) && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| id.to_ascii_uppercase())
}

/// A PARAM.SFO holding `entries`, each a text value, as a PSP writes one: for tests, here and in
/// what reads saves and games.
pub fn param_sfo(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut keys = Vec::new();
    let mut data = Vec::new();
    let mut table = Vec::new();
    for (key, value) in entries {
        let key_at = keys.len() as u16;
        keys.extend_from_slice(key.as_bytes());
        keys.push(0);
        let value_at = data.len() as u32;
        let mut v = value.as_bytes().to_vec();
        v.push(0);
        let len = v.len() as u32;
        data.extend_from_slice(&v);
        table.extend_from_slice(&key_at.to_le_bytes());
        table.extend_from_slice(&0x0204u16.to_le_bytes());
        table.extend_from_slice(&len.to_le_bytes());
        table.extend_from_slice(&len.to_le_bytes());
        table.extend_from_slice(&value_at.to_le_bytes());
    }
    let keys_at = 20 + table.len() as u32;
    let data_at = keys_at + keys.len() as u32;
    let mut out = b"\0PSF".to_vec();
    out.extend_from_slice(&0x0101u32.to_le_bytes());
    out.extend_from_slice(&keys_at.to_le_bytes());
    out.extend_from_slice(&data_at.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend(table);
    out.extend(keys);
    out.extend(data);
    out
}

/// The largest PARAM.SFO worth reading: real ones are a kilobyte or two.
const SFO_MAX: u32 = 16 * 1024;

/// PARAM.SFO from a disc whose sectors `sector` reads: the primary volume descriptor at sector 16
/// leads to the root folder, the root to `PSP_GAME`, and that to `PARAM.SFO`.
fn disc_sfo(sector: &mut dyn FnMut(u32) -> Option<[u8; USER]>) -> Option<Vec<u8>> {
    let pvd = sector(16)?;
    if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
        return None;
    }
    let root = &pvd[156..156 + 34];
    let (root_lba, root_len) = (le32(&root[2..]), le32(&root[10..]));
    let game = find(sector, root_lba, root_len, "PSP_GAME", true)?;
    let (lba, len) = find(sector, game.0, game.1, "PARAM.SFO", false)?;
    let len = len.min(SFO_MAX);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len.div_ceil(USER as u32) {
        out.extend_from_slice(&sector(lba.checked_add(i)?)?);
    }
    out.truncate(len as usize);
    Some(out)
}

/// The sector and length of `name`, a folder or a file as `folder` says, in the folder at
/// sector `lba`, `len` bytes long.
fn find(
    sector: &mut dyn FnMut(u32) -> Option<[u8; USER]>,
    lba: u32,
    len: u32,
    name: &str,
    folder: bool,
) -> Option<(u32, u32)> {
    // a folder of a PSP disc is a sector or two; more than 16 is not a real one
    let sectors = len.div_ceil(USER as u32).min(16);
    (0..sectors).find_map(|i| find_entry(&sector(lba.checked_add(i)?)?, name, folder))
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn iso_sector(file: &mut File, lba: u32) -> Option<[u8; USER]> {
    let mut out = [0u8; USER];
    file.seek(SeekFrom::Start(lba as u64 * USER as u64)).ok()?;
    file.read_exact(&mut out).ok()?;
    Some(out)
}

/// PARAM.SFO from the start of an eboot: its header gives where it is and where what follows it
/// starts.
fn pbp_sfo(path: &Path) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut head = [0u8; 16];
    file.read_exact(&mut head).ok()?;
    if &head[..4] != b"\0PBP" {
        return None;
    }
    let (at, next) = (le32(&head[8..]), le32(&head[12..]));
    let len = next.checked_sub(at)?.min(SFO_MAX);
    let mut sfo = vec![0u8; len as usize];
    file.seek(SeekFrom::Start(at as u64)).ok()?;
    file.read_exact(&mut sfo).ok()?;
    Some(sfo)
}

/// A CISO image: the disc in blocks, each deflated or, where its index's top bit is set, kept as
/// it is. What hashes a game for its achievements reads one this way too.
pub struct Cso {
    file: File,
    block: u32,
    align: u8,
    index: Vec<u32>,
}

/// A CISO header: the magic, its size, the disc's size, the block size, the version, the
/// alignment of the blocks, and two bytes unused.
const CSO_HEADER: usize = 24;

impl Cso {
    pub fn open(path: &Path) -> Option<Cso> {
        let mut file = File::open(path).ok()?;
        let mut head = [0u8; CSO_HEADER];
        file.read_exact(&mut head).ok()?;
        if &head[..4] != b"CISO" {
            return None;
        }
        let total = u64::from_le_bytes(head[8..16].try_into().ok()?);
        let block = le32(&head[16..]);
        if block == 0 || !(block as usize).is_multiple_of(USER) {
            return None;
        }
        let blocks = total.div_ceil(block as u64) as usize;
        // the first 32 blocks' worth of index reach well past PARAM.SFO; no more is needed
        let wanted = blocks.min(4096) + 1;
        let mut raw = vec![0u8; wanted * 4];
        file.read_exact(&mut raw).ok()?;
        Some(Cso {
            file,
            block,
            align: head[21],
            index: raw.chunks_exact(4).map(le32).collect(),
        })
    }

    /// The disc's sector `lba`, its 2048 bytes; `None` past its end.
    pub fn sector(&mut self, lba: u32) -> Option<[u8; USER]> {
        let at = lba as u64 * USER as u64;
        let n = (at / self.block as u64) as usize;
        let (this, next) = (*self.index.get(n)?, *self.index.get(n + 1)?);
        let plain = this & 0x8000_0000 != 0;
        let start = ((this & 0x7fff_ffff) as u64) << self.align;
        let end = ((next & 0x7fff_ffff) as u64) << self.align;
        let mut stored = vec![0u8; end.checked_sub(start)? as usize];
        self.file.seek(SeekFrom::Start(start)).ok()?;
        self.file.read_exact(&mut stored).ok()?;
        let block = match plain {
            true => stored,
            false => miniz_oxide::inflate::decompress_to_vec(&stored).ok()?,
        };
        let within = (at % self.block as u64) as usize;
        block.get(within..within + USER)?.try_into().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PARAM.SFO with `DISC_ID` and `TITLE`.
    pub(crate) fn sfo(id: &str, title: &str) -> Vec<u8> {
        param_sfo(&[("DISC_ID", id), ("TITLE", title)])
    }

    #[test]
    fn a_save_s_param_sfo_names_its_game_and_its_folder_the_game_s_id() {
        let sfo = param_sfo(&[
            ("TITLE", "Monster Hunter Freedom Unite"),
            ("SAVEDATA_TITLE", "Save"),
        ]);
        assert_eq!(
            save_title(&sfo).as_deref(),
            Some("Monster Hunter Freedom Unite")
        );
        assert_eq!(from_sfo(&sfo), None, "a save has no disc ID");
        assert_eq!(save_id("ULUS10391DATA00").as_deref(), Some("ULUS10391"));
        assert_eq!(save_id("ulus10391"), Some("ULUS10391".into()));
        assert_eq!(save_id("SYSTEM"), None);
        assert_eq!(save_id("ULUSX0391DATA"), None);
    }

    /// A folder record for `name` at sector `lba`, `len` long.
    fn record(name: &str, lba: u32, len: u32, folder: bool) -> Vec<u8> {
        let name = name.as_bytes();
        let size = 33 + name.len() + (1 - name.len() % 2);
        let mut r = vec![0u8; size];
        r[0] = size as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[10..14].copy_from_slice(&len.to_le_bytes());
        r[25] = if folder { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    /// A UMD image of 24 sectors: the descriptor at 16, the root at 18, PSP_GAME at 19 and its
    /// PARAM.SFO at 20.
    pub(crate) fn umd(id: &str, title: &str) -> Vec<u8> {
        let mut img = vec![0u8; 24 * USER];
        let pvd = 16 * USER;
        img[pvd] = 1;
        img[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        img[pvd + 156..pvd + 190].copy_from_slice(&record("\0", 18, USER as u32, true)[..34]);
        let root = record("PSP_GAME", 19, USER as u32, true);
        img[18 * USER..18 * USER + root.len()].copy_from_slice(&root);
        let sfo = sfo(id, title);
        let file = record("PARAM.SFO;1", 20, sfo.len() as u32, false);
        img[19 * USER..19 * USER + file.len()].copy_from_slice(&file);
        img[20 * USER..20 * USER + sfo.len()].copy_from_slice(&sfo);
        img
    }

    /// `img` as a CISO of 2048-byte blocks, every other one kept as it is.
    fn cso(img: &[u8]) -> Vec<u8> {
        let blocks: Vec<&[u8]> = img.chunks(USER).collect();
        let mut out = b"CISO".to_vec();
        out.extend_from_slice(&(CSO_HEADER as u32).to_le_bytes());
        out.extend_from_slice(&(img.len() as u64).to_le_bytes());
        out.extend_from_slice(&(USER as u32).to_le_bytes());
        out.extend_from_slice(&[1, 0, 0, 0]);
        let mut at = (CSO_HEADER + (blocks.len() + 1) * 4) as u32;
        let mut index = Vec::new();
        let mut body = Vec::new();
        for (i, b) in blocks.iter().enumerate() {
            let stored = match i % 2 {
                0 => miniz_oxide::deflate::compress_to_vec(b, 6),
                _ => b.to_vec(),
            };
            index.push(at | if i % 2 == 1 { 0x8000_0000 } else { 0 });
            at += stored.len() as u32;
            body.extend(stored);
        }
        index.push(at);
        for i in index {
            out.extend_from_slice(&i.to_le_bytes());
        }
        out.extend(body);
        out
    }

    #[test]
    fn a_games_id_says_its_region() {
        use crate::Region;
        assert_eq!(id_region("ULUS10041"), Some(Region::Usa));
        assert_eq!(id_region("UCES00001"), Some(Region::Europe));
        assert_eq!(id_region("ULJM05800"), Some(Region::Japan));
        assert_eq!(id_region("NPUH10001"), Some(Region::Usa));
        assert_eq!(id_region("ULKS46163"), Some(Region::Other));
        assert_eq!(id_region("X"), None);
    }

    #[test]
    fn a_umd_image_names_its_game() {
        let d = tempfile::tempdir().unwrap();
        let iso = d.path().join("Daxter.iso");
        std::fs::write(&iso, umd("UCUS98618", "Daxter")).unwrap();
        assert_eq!(
            info(&iso),
            Some(Sfo {
                id: "UCUS98618".into(),
                title: "Daxter".into()
            })
        );
    }

    #[test]
    fn a_compressed_image_names_its_game_too() {
        let d = tempfile::tempdir().unwrap();
        let cso_path = d.path().join("Lumines.cso");
        std::fs::write(&cso_path, cso(&umd("ULUS10046", "LUMINES"))).unwrap();
        assert_eq!(info(&cso_path).map(|s| s.id), Some("ULUS10046".into()));
    }

    #[test]
    fn an_eboot_names_its_game_from_its_param_sfo() {
        let d = tempfile::tempdir().unwrap();
        let sfo = sfo("NPUH10001", "Echochrome");
        let mut pbp = b"\0PBP".to_vec();
        pbp.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        pbp.extend_from_slice(&40u32.to_le_bytes());
        pbp.extend_from_slice(&(40 + sfo.len() as u32).to_le_bytes());
        pbp.resize(40, 0);
        pbp.extend(sfo);
        let path = d.path().join("Echochrome.pbp");
        std::fs::write(&path, pbp).unwrap();
        assert_eq!(info(&path).map(|s| s.title), Some("Echochrome".into()));
    }

    #[test]
    fn what_is_not_a_umd_names_nothing() {
        let d = tempfile::tempdir().unwrap();
        let junk = d.path().join("junk.iso");
        std::fs::write(&junk, vec![7u8; 24 * USER]).unwrap();
        assert_eq!(info(&junk), None);
        let chd = d.path().join("game.chd");
        std::fs::write(&chd, umd("ULUS10041", "Lumines")).unwrap();
        assert_eq!(info(&chd), None, "a chd is compressed another way");
    }
}
