//! Bounded new vector artwork. Covers pixels without erasing underlying data.
use super::{Budget, appearance, overlay};
use crate::{Error, PdfDocument, Result};
use lopdf::{
    Dictionary, Object, Stream,
    content::{Content, Operation},
    dictionary,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum VectorShape {
    Rectangle { rectangle: [f64; 4] },
    Ellipse { rectangle: [f64; 4] },
    Line { from: [f64; 2], to: [f64; 2] },
    Polyline { points: Vec<[f64; 2]> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorStroke {
    pub color: [f64; 3],
    pub width: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorOverlay {
    /// One-based page; unrotated PDF user-space coordinates.
    pub page: u32,
    pub shape: VectorShape,
    pub stroke: Option<VectorStroke>,
    pub fill: Option<[f64; 3]>,
    pub opacity: f64,
}
fn real(n: f64) -> Object {
    Object::Real(n as f32)
}
fn op(name: &str, values: impl IntoIterator<Item = f64>) -> Operation {
    Operation::new(name, values.into_iter().map(real).collect())
}
fn color(c: &[f64; 3]) -> bool {
    c.iter().all(|n| n.is_finite() && (0.0..=1.0).contains(n))
}
fn point(p: [f64; 2]) -> bool {
    p.iter().all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
}
impl VectorOverlay {
    fn planned(&self) -> Result<([f64; 4], Vec<Operation>)> {
        if self.stroke.is_none() && self.fill.is_none()
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
            || self.opacity as f32 == 0.0
        {
            return Err(Error::Invalid(
                "vector overlay needs paint and opacity in (0,1] representable at PDF number precision",
            ));
        }
        if self.fill.as_ref().is_some_and(|c| !color(c))
            || self.stroke.as_ref().is_some_and(|s| {
                !color(&s.color) || !s.width.is_finite() || !(0.01..=1024.0).contains(&s.width)
            })
        {
            return Err(Error::Invalid("vector color or stroke width is invalid"));
        }
        let mut ops = vec![
            op("q", []),
            Operation::new("gs", vec![Object::Name(b"Paint".to_vec())]),
        ];
        if let Some(s) = &self.stroke {
            ops.extend([
                op("RG", s.color),
                op("w", [s.width]),
                Operation::new("J", vec![Object::Integer(1)]),
                Operation::new("j", vec![Object::Integer(1)]),
            ]);
        }
        if let Some(c) = self.fill {
            ops.push(op("rg", c));
        }
        let bounds = match &self.shape {
            VectorShape::Rectangle { rectangle: r } | VectorShape::Ellipse { rectangle: r } => {
                let [x0, y0, x1, y1] = *r;
                if !point([x0, y0]) || !point([x1, y1]) || x0 >= x1 || y0 >= y1 {
                    return Err(Error::Invalid("vector rectangle is invalid"));
                }
                if matches!(self.shape, VectorShape::Rectangle { .. }) {
                    ops.push(op("re", [x0, y0, x1 - x0, y1 - y0]));
                } else {
                    let cx = (x0 + x1) / 2.0;
                    let cy = (y0 + y1) / 2.0;
                    let rx = (x1 - x0) / 2.0;
                    let ry = (y1 - y0) / 2.0;
                    let k = 0.552_284_749_830_793_6;
                    ops.extend([
                        op("m", [x1, cy]),
                        op("c", [x1, cy + k * ry, cx + k * rx, y1, cx, y1]),
                        op("c", [cx - k * rx, y1, x0, cy + k * ry, x0, cy]),
                        op("c", [x0, cy - k * ry, cx - k * rx, y0, cx, y0]),
                        op("c", [cx + k * rx, y0, x1, cy - k * ry, x1, cy]),
                        op("h", []),
                    ]);
                }
                *r
            }
            VectorShape::Line { from, to } => {
                if self.fill.is_some() || !point(*from) || !point(*to) || from == to {
                    return Err(Error::Invalid(
                        "line requires distinct finite points and stroke only",
                    ));
                }
                ops.extend([op("m", *from), op("l", *to)]);
                [
                    from[0].min(to[0]),
                    from[1].min(to[1]),
                    from[0].max(to[0]),
                    from[1].max(to[1]),
                ]
            }
            VectorShape::Polyline { points } => {
                if points.len() > 4096 {
                    return Err(Error::Limit("freehand overlay exceeds 4096 points"));
                }
                if self.fill.is_some()
                    || points.len() < 2
                    || points.iter().any(|p| !point(*p))
                    || points.windows(2).all(|w| w[0] == w[1])
                {
                    return Err(Error::Invalid(
                        "freehand requires 2..4096 finite points and stroke only",
                    ));
                }
                let first = points[0];
                let mut b = [first[0], first[1], first[0], first[1]];
                ops.push(op("m", first));
                for p in &points[1..] {
                    ops.push(op("l", *p));
                    b = [
                        b[0].min(p[0]),
                        b[1].min(p[1]),
                        b[2].max(p[0]),
                        b[3].max(p[1]),
                    ];
                }
                b
            }
        };
        ops.push(op(
            match (self.stroke.is_some(), self.fill.is_some()) {
                (true, true) => "B",
                (true, false) => "S",
                _ => "f",
            },
            [],
        ));
        ops.push(op("Q", []));
        let margin = self.stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
        Ok((
            [
                bounds[0] - margin,
                bounds[1] - margin,
                bounds[2] + margin,
                bounds[3] + margin,
            ],
            ops,
        ))
    }
}
impl PdfDocument {
    /// Add bounded vector artwork, preserving source content. This is not redaction.
    pub fn add_vector_overlay(&mut self, request: &VectorOverlay) -> Result<()> {
        if self.encrypted_input || self.info()?.signature_fields != 0 {
            return Err(Error::Refused(
                "signed or decrypted documents are read-only",
            ));
        }
        self.validate_selection(&[request.page])?;
        let (bounds, operations) = request.planned()?;
        let encoded_bounds = bounds.map(|value| value as f32);
        if encoded_bounds[0] >= encoded_bounds[2] || encoded_bounds[1] >= encoded_bounds[3] {
            return Err(Error::Invalid(
                "vector paint bounds collapse at PDF number precision",
            ));
        }
        let info = self.info()?;
        let geometry = &info.pages[request.page as usize - 1];
        for b in [geometry.media_box, geometry.crop_box] {
            if bounds[0] < f64::from(b[0])
                || bounds[1] < f64::from(b[1])
                || bounds[2] > f64::from(b[2])
                || bounds[3] > f64::from(b[3])
            {
                return Err(Error::Refused(
                    "vector paint including stroke must fit within the visible page",
                ));
            }
        }
        let id = self.pages[request.page as usize - 1];
        let mut budget = Budget::new();
        let catalog = budget
            .resolve(&self.document, self.document.trailer.get(b"Root")?)?
            .1
            .as_dict()?;
        if catalog.has(b"StructTreeRoot")
            || self
                .document
                .get_object(id)?
                .as_dict()?
                .has(b"StructParents")
        {
            return Err(Error::Refused(
                "vector overlays on tagged pages need semantic tagging support",
            ));
        }
        let mut source = Vec::new();
        let mut ids = Vec::new();
        if let Ok(v) = self.document.get_object(id)?.as_dict()?.get(b"Contents") {
            overlay::contents(
                &self.document,
                v,
                &mut budget,
                &mut BTreeSet::new(),
                &mut ids,
                &mut source,
                0,
            )?;
        }
        if overlay::balanced(&source)?
            .saturating_add(operations.len())
            .saturating_add(6)
            > 100_000
        {
            return Err(Error::Limit("vector overlay exceeds 100000 operators"));
        }
        let resources = crate::structure::inherited_raw(&self.document, id, b"Resources")?
            .map(|v| {
                budget
                    .resolve(&self.document, v)
                    .and_then(|(_, v)| Ok(v.as_dict()?))
            })
            .transpose()?;
        let xobjects = resources
            .and_then(|r| r.get(b"XObject").ok())
            .map(|v| {
                budget
                    .resolve(&self.document, v)
                    .and_then(|(_, v)| Ok(v.as_dict()?))
            })
            .transpose()?;
        for dictionary in [resources, xobjects].into_iter().flatten() {
            for (k, v) in dictionary.iter() {
                budget.report(k.len())?;
                appearance::copied(v, 0, &mut budget)?;
            }
        }
        let mut name = b"NativeVectorOverlay".to_vec();
        for index in 0..=100_000 {
            if !xobjects.is_some_and(|x| x.has(&name)) {
                break;
            }
            if index == 100_000 {
                return Err(Error::Limit("vector resource name budget"));
            }
            name = format!("NativeVectorOverlay{index}").into_bytes();
        }
        if self.document.objects.len().saturating_add(3) > crate::MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(3) >= crate::MAX_OBJECTS
        {
            return Err(Error::Limit("vector object allocation budget"));
        }
        let encoded = Content { operations }.encode()?;
        if encoded.len() > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("vector stream exceeds 32 MiB"));
        }
        let mut new_resources = resources.cloned().unwrap_or_default();
        let mut new_xobjects = xobjects.cloned().unwrap_or_default();
        let mut candidate = self.document.clone();
        crate::require_pdf_1_4(&mut candidate);
        let form=candidate.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>bounds.into_iter().map(real).collect::<Vec<_>>(),"Resources"=>dictionary!{"ExtGState"=>dictionary!{"Paint"=>dictionary!{"Type"=>"ExtGState","CA"=>real(request.opacity),"ca"=>real(request.opacity),"BM"=>"Normal"}}}},encoded));
        new_xobjects.set(name.clone(), form);
        new_resources.set("XObject", new_xobjects);
        let prefix = candidate.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
        let suffix = candidate.add_object(Stream::new(
            Dictionary::new(),
            Content {
                operations: vec![
                    // A pending W/W* clipping operation is consumed when the
                    // current path ends. End it inside the owned source state,
                    // before Q restores the unclipped overlay graphics state.
                    op("n", []),
                    op("Q", []),
                    op("q", []),
                    Operation::new("Do", vec![Object::Name(name)]),
                    op("Q", []),
                ],
            }
            .encode()?,
        ));
        let mut new_contents = vec![Object::Reference(prefix)];
        new_contents.extend(ids.into_iter().map(Object::Reference));
        new_contents.push(Object::Reference(suffix));
        let page = candidate.get_object_mut(id)?.as_dict_mut()?;
        page.set("Resources", new_resources);
        page.set("Contents", new_contents);
        let rebuilt = Self::from_document(candidate, false)?;
        if rebuilt.info()?.pages != info.pages {
            return Err(Error::Invalid("vector overlay changed page geometry"));
        }
        rebuilt.verified_bytes()?;
        *self = rebuilt;
        Ok(())
    }
}
