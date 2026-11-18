use super::*;

/// The keyed (`Shape::Map`) edit fields, handed to `$m` (which makes a match arm, or a term, of
/// each): `set_fields`, `patch_fields`, `apply` and `is_empty` take theirs from here.
macro_rules! keyed_fields {
    ($m:ident) => {
        $m!(renames notes npcs plots objects cleared sprites designs crossings buildings towns ground plateaus waters wharves cultures hexes streets links paths members fences fence_gaps)
    };
}

impl Edits {
    /// `next` replacing these edits whole, keeping listed entries' places (`created`: later ids
    /// are indices): the entries `next` lacks stay, marked removed, as the ops that make it leave
    /// them wherever they are applied (the app's `keepPlaces`).
    pub fn keeping_places(&self, mut next: Edits) -> Edits {
        let n = next.created.len();
        if self.created.len() > n {
            next.created.extend(self.created[n..].iter().map(|c| Created { removed: true, ..c.clone() }));
        }
        next
    }

    /// Site `site`'s levels renumbered (they count from the bottom) after `delta` levels were dug
    /// below its deepest (or, below 0, its deepest filled in): the names, notes and hidden marks
    /// of its levels and rooms, plots' anchors on them and NPCs inside follow their levels; those
    /// of levels filled in go (an NPC stays in the site, on its level as it is now).
    pub fn shift_levels(&mut self, site: &str, delta: isize) {
        if delta == 0 {
            return;
        }
        // Some(new id, or None if its level is gone) for the site's level and room ids.
        let moved = |id: &str| -> Option<Option<String>> {
            if let Some((s, li, own)) = crate::model::ids::split_point_id(id).filter(|(s, _, _)| *s == site) {
                let li = li as isize + delta;
                return Some((li >= 0).then(|| format!("m:{s}:{li}:{own}")));
            }
            let (s, li, ri) = crate::model::ids::split_room_id(id).filter(|(s, _, _)| *s == site)?;
            let li = li as isize + delta;
            Some((li >= 0).then(|| match ri {
                Some(ri) => format!("r:{s}:{li}:{ri}"),
                None => format!("l:{s}:{li}"),
            }))
        };
        fn rekey<V>(m: &mut BTreeMap<String, V>, moved: &dyn Fn(&str) -> Option<Option<String>>) {
            let ids: Vec<String> = m.keys().filter(|k| moved(k).is_some()).cloned().collect();
            let taken: Vec<(Option<String>, V)> = ids.iter().map(|k| (moved(k).flatten(), m.remove(k).expect("listed"))).collect();
            for (k, v) in taken {
                if let Some(k) = k {
                    m.insert(k, v);
                }
            }
        }
        rekey(&mut self.renames, &moved);
        rekey(&mut self.notes, &moved);
        let hidden: Vec<String> = self.hidden.iter().filter(|k| moved(k).is_some()).cloned().collect();
        for k in hidden {
            self.hidden.remove(&k);
            if let Some(k) = moved(&k).flatten() {
                self.hidden.insert(k);
            }
        }
        for p in self.plots.values_mut() {
            if p.anchors.iter().any(|a| moved(a).is_some()) {
                p.anchors = p.anchors.iter().filter_map(|a| moved(a).unwrap_or_else(|| Some(a.clone()))).collect();
            }
        }
        for n in self.npcs.values_mut() {
            if let Some(at) = n.location.as_mut().filter(|at| at.id == site)
                && let Some(li) = at.level
            {
                at.level = Some((li as isize + delta).max(0) as usize);
            }
        }
        // (A link with an end on a level filled in goes with it.)
        let gone: Vec<String> = self
            .links
            .iter_mut()
            .filter_map(|(k, l)| {
                for e in [&mut l.a, &mut l.b] {
                    match moved(&e.level) {
                        Some(Some(id)) => e.level = id,
                        Some(None) => return Some(k.clone()),
                        None => {}
                    }
                }
                None
            })
            .collect();
        for k in gone {
            self.links.remove(&k);
        }
    }

    /// The live site `c` (being created, asked for at `asked`) would duplicate, if any
    /// (`Created::same_site`).
    pub fn existing_site(&self, c: &Created, asked: [f64; 2]) -> Option<&Created> {
        self.created.iter().find(|o| c.same_site(o, asked))
    }

    /// Replace the fields given (a JSON object of some of the edits' fields, each whole), the
    /// rest staying: a big world's live edits arrive field by field. Returns the created sites as
    /// they were when `created` was among them (only then is generation touched).
    pub fn set_fields(&mut self, json: &str) -> Result<Option<Vec<Created>>, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let keys: Vec<String> = v.as_object().ok_or("edits fields: a JSON object")?.keys().cloned().collect();
        let mut part: Edits = serde_json::from_value(v).map_err(|e| e.to_string())?;
        let mut old = None;
        for k in &keys {
            macro_rules! arms {
                ($($f:ident)*) => {
                    match k.as_str() {
                        $(stringify!($f) => self.$f = std::mem::take(&mut part.$f),)*
                        "hidden" => self.hidden = std::mem::take(&mut part.hidden),
                        "created" => old = Some(std::mem::replace(&mut self.created, std::mem::take(&mut part.created))),
                        "removed_roads" => self.removed_roads = std::mem::take(&mut part.removed_roads),
                        _ => return Err(format!("no such edits field: {k}")),
                    }
                };
            }
            keyed_fields!(arms);
        }
        Ok(old)
    }

    /// Change some entries of keyed fields: `{field: {set: {key: value…}, unset: [key…]}}`
    /// (a brush stroke in a world of thousands of objects sends only its own).
    pub fn patch_fields(&mut self, json: &str) -> Result<(), String> {
        fn patch<T: serde::de::DeserializeOwned>(m: &mut BTreeMap<String, T>, p: &Value) -> Result<(), String> {
            for (k, v) in p["set"].as_object().into_iter().flatten() {
                m.insert(k.clone(), serde_json::from_value(v.clone()).map_err(|e| format!("{k}: {e}"))?);
            }
            for k in p["unset"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                m.remove(k);
            }
            Ok(())
        }
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        for (field, p) in v.as_object().ok_or("edits patch: a JSON object")? {
            macro_rules! arms {
                ($($f:ident)*) => {
                    match field.as_str() {
                        $(stringify!($f) => patch(&mut self.$f, p)?,)*
                        _ => return Err(format!("{field}: not a keyed edits field")),
                    }
                };
            }
            keyed_fields!(arms);
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        macro_rules! all_empty {
            ($($f:ident)*) => {
                $(self.$f.is_empty() &&)* self.hidden.is_empty() && self.created.is_empty() && self.removed_roads.is_empty()
            };
        }
        keyed_fields!(all_empty)
    }

    /// The ops that turn these edits into `to`.
    pub fn diff(&self, to: &Edits) -> Vec<EditOp> {
        let (a, b) = (serde_json::to_value(self).unwrap_or_default(), serde_json::to_value(to).unwrap_or_default());
        let mut ops = Vec::new();
        for &(field, shape) in EDIT_FIELDS {
            let (ea, eb) = (entries(&a, field, shape), entries(&b, field, shape));
            // Each side's entries by key, the first of a repeated key kept (as a search would find it).
            fn by_key(e: &[(String, Value)]) -> FastMap<&str, &Value> {
                let mut m = FastMap::default();
                for (k, v) in e {
                    m.entry(k.as_str()).or_insert(v);
                }
                m
            }
            let (ma, mb) = (by_key(&ea), by_key(&eb));
            // (A list's entries in list order: an entry appended can only follow the one before it.)
            let keys: Vec<&String> = match shape {
                Shape::List => {
                    let mut k: Vec<&String> = eb.iter().map(|(k, _)| k).collect();
                    k.extend(ea.iter().map(|(k, _)| k).filter(|k| !mb.contains_key(k.as_str())));
                    k
                }
                _ => ea.iter().chain(&eb).map(|(k, _)| k).collect::<BTreeSet<_>>().into_iter().collect(),
            };
            for key in keys {
                let (va, vb) = (ma.get(key.as_str()).copied(), mb.get(key.as_str()).copied());
                if va == vb {
                    continue;
                }
                ops.push(match vb {
                    Some(v) => EditOp::Set { field: field.into(), key: key.clone(), value: v.clone() },
                    None => EditOp::Unset { field: field.into(), key: key.clone() },
                });
            }
        }
        ops
    }

    /// Apply one op; returns the op that undoes it. Nothing changes on an error.
    pub fn apply(&mut self, op: &EditOp) -> Result<EditOp, String> {
        let (field, key) = match op {
            EditOp::Set { field, key, .. } | EditOp::Unset { field, key } => (field.as_str(), key.clone()),
        };
        let &(_, shape) = EDIT_FIELDS.iter().find(|(f, _)| *f == field).ok_or_else(|| format!("no such edits field: {field}"))?;
        // Keyed fields change one entry in place (a brush stroke is dozens of ops in a world of
        // thousands of entries); lists and sets go through the whole edits as JSON.
        fn one<T: Serialize + serde::de::DeserializeOwned>(m: &mut BTreeMap<String, T>, op: &EditOp, field: &str, key: &str) -> Result<Option<Value>, String> {
            let before = m.get(key).map(|v| serde_json::to_value(v).map_err(|e| e.to_string())).transpose()?;
            match op {
                EditOp::Set { value, .. } => {
                    m.insert(key.to_string(), serde_json::from_value(value.clone()).map_err(|e| format!("{field} {key}: {e}"))?);
                }
                EditOp::Unset { .. } => {
                    m.remove(key);
                }
            }
            Ok(before)
        }
        macro_rules! arms {
            ($($f:ident)*) => {
                match field {
                    $(stringify!($f) => Some(one(&mut self.$f, op, field, &key)),)*
                    _ => None,
                }
            };
        }
        let fast = keyed_fields!(arms);
        if let Some(before) = fast {
            let field = field.to_string();
            return Ok(match before? {
                Some(value) => EditOp::Set { field, key, value },
                None => EditOp::Unset { field, key },
            });
        }
        let mut v = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let mut list = entries(&v, field, shape);
        let at = list.iter().position(|(k, _)| *k == key);
        let before = at.map(|i| list[i].1.clone());
        match (op, shape) {
            (EditOp::Set { value, .. }, Shape::List) => {
                let i = list_index(&key).ok_or_else(|| format!("{field} keys are <prefix>:<index>, not {key}"))?;
                if i > list.len() {
                    return Err(format!("{key}: {field} has only {} entries", list.len()));
                }
                if i == list.len() {
                    list.push((key.clone(), value.clone()));
                } else {
                    list[i].1 = value.clone();
                }
            }
            (EditOp::Set { value, .. }, _) => match at {
                Some(i) => list[i].1 = value.clone(),
                None => list.push((key.clone(), value.clone())),
            },
            // Listed entries keep their place (later ids are indices): unset marks one removed.
            (EditOp::Unset { .. }, Shape::List) => {
                if let Some(i) = at
                    && let Some(m) = list[i].1.as_object_mut()
                {
                    m.insert("removed".into(), Value::Bool(true));
                }
            }
            (EditOp::Unset { .. }, _) => list.retain(|(k, _)| *k != key),
        }
        put(&mut v, field, shape, list);
        *self = serde_json::from_value(v).map_err(|e| format!("{field} {key}: {e}"))?;
        let field = field.to_string();
        Ok(match before {
            Some(value) => EditOp::Set { field, key, value },
            None => EditOp::Unset { field, key },
        })
    }
}

/// One change to the edits: one entry of one field set or removed. The app and `mapd` exchange
/// these (not whole edits), so changes made at the same time to different entries all stay.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    Set { field: String, key: String, value: Value },
    Unset { field: String, key: String },
}

#[derive(Clone, Copy)]
enum Shape {
    /// An object keyed by id.
    Map,
    /// A sorted list of ids (an entry's value is `true`).
    Set,
    /// A list whose entries' ids are `<prefix>:<index>` (never shortened).
    List,
}

/// Layout numbers from here up are changes set aside: when the world is remade (a sketch
/// redrawn), the app moves the keys of a place that came out differently, or is gone, here
/// (`t0::remap`). They stay in the file and resolve nowhere.
pub const PARKED_LAYOUT: usize = 1_000_000;

/// The layout number in a layout-local id (`b:<layout>:…`, `d:`, `t:`, `u:`, `k:`, `w:`, a
/// level's, room's or point note's `l:`/`r:`/`m:` round one), if it has one.
pub fn layout_in_id(id: &str) -> Option<usize> {
    let id = id.strip_prefix("l:").or_else(|| id.strip_prefix("r:")).or_else(|| id.strip_prefix("m:")).unwrap_or(id);
    let (head, rest) = id.split_once(':')?;
    if !matches!(head, "b" | "d" | "t" | "u" | "k" | "w") {
        return None;
    }
    rest.split(':').next()?.parse().ok()
}

/// The edit fields and how their entries are keyed (mirrored in `app/src/sync/ops.ts`). A field
/// whose keys or values hold layout numbers must also join `t0::remap::LAYOUT_ID_FIELDS` (and the
/// app's mirror), so a remade world's places keep them.
const EDIT_FIELDS: &[(&str, Shape)] = &[
    ("renames", Shape::Map),
    ("notes", Shape::Map),
    ("hidden", Shape::Set),
    ("created", Shape::List),
    ("npcs", Shape::Map),
    ("plots", Shape::Map),
    ("objects", Shape::Map),
    ("cleared", Shape::Map),
    ("sprites", Shape::Map),
    ("designs", Shape::Map),
    ("crossings", Shape::Map),
    ("buildings", Shape::Map),
    ("towns", Shape::Map),
    ("ground", Shape::Map),
    ("plateaus", Shape::Map),
    ("waters", Shape::Map),
    ("wharves", Shape::Map),
    ("cultures", Shape::Map),
    ("hexes", Shape::Map),
    ("streets", Shape::Map),
    ("links", Shape::Map),
    ("paths", Shape::Map),
    ("members", Shape::Map),
    ("removed_roads", Shape::Set),
    ("fences", Shape::Map),
    ("fence_gaps", Shape::Map),
];

fn list_index(key: &str) -> Option<usize> {
    key.rsplit(':').next()?.parse().ok()
}

/// A field's entries as (key, value), in order.
fn entries(v: &Value, field: &str, shape: Shape) -> Vec<(String, Value)> {
    match (shape, &v[field]) {
        (Shape::Map, Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        (Shape::Set, Value::Array(a)) => a.iter().filter_map(|k| Some((k.as_str()?.to_string(), Value::Bool(true)))).collect(),
        (Shape::List, Value::Array(a)) => a.iter().enumerate().map(|(i, e)| (e["id"].as_str().map(str::to_string).unwrap_or_else(|| format!("c:{i}")), e.clone())).collect(),
        _ => Vec::new(),
    }
}

fn put(v: &mut Value, field: &str, shape: Shape, list: Vec<(String, Value)>) {
    v[field] = match shape {
        Shape::Map => Value::Object(list.into_iter().collect()),
        Shape::Set => Value::Array(list.into_iter().map(|(k, _)| Value::String(k)).collect()),
        Shape::List => Value::Array(list.into_iter().map(|(_, e)| e).collect()),
    };
}
