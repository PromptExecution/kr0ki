//! The Kroki-family diagram formats kr0ki's P0 loop accepts.
//!
//! Deliberately *only* the formats a stock Kroki container renders with no headless-
//! Chromium companion (PRD-KR0KI-001 NFR3). `mermaid`, `bpmn`, `excalidraw`, and
//! `diagramsnet` are left out on purpose — live-verified (2026-09-16) that a
//! companion-free Kroki container answers all four with `503 Service Unavailable`,
//! while every format below returns a real render. If Kroki adds a new companion-
//! dependent format in future, re-verify it live before adding here rather than
//! assuming from its docs — `503` is the authoritative signal, not the format name.
//!
//! `DiagramFormat` is a newtype over the Kroki slug, not an enum: the
//! [`diagram_formats!`] macro below generates both the named associated consts and
//! `ALL` from **one** list, so there is exactly one place to add a format instead of
//! four. The inner `&'static str` stays private — construction is only possible
//! through a declared const or [`FromStr`], which is what keeps this a closed
//! whitelist rather than an arbitrary pass-through. Don't make the field `pub`.

use std::str::FromStr;

/// A diagram source language kr0ki can hand to a Kroki backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagramFormat(&'static str);

/// Declares `DiagramFormat::$name` consts and `DiagramFormat::ALL` together from one
/// list, so adding a format can't compile with the const declared but forgotten from
/// `ALL` — there is only one list to edit.
macro_rules! diagram_formats {
    ($($name:ident => $slug:literal),+ $(,)?) => {
        // These consts are deliberately PascalCase, matching how the old enum
        // variants read at call sites (`DiagramFormat::D2`, `DiagramFormat::TikZ`) —
        // not an oversight of Rust's SCREAMING_CASE convention for constants.
        #[allow(non_upper_case_globals)]
        impl DiagramFormat {
            $(
                pub const $name: Self = Self($slug);
            )+

            /// Every supported format, for callers that enumerate the set (e.g. a
            /// `/formats` endpoint).
            pub const ALL: &'static [DiagramFormat] = &[$(Self::$name),+];
        }
    };
}

diagram_formats! {
    PlantUml => "plantuml",
    C4PlantUml => "c4plantuml",
    GraphViz => "graphviz",
    D2 => "d2",
    VegaLite => "vegalite",
    Ditaa => "ditaa",
    Nomnoml => "nomnoml",
    WaveDrom => "wavedrom",
    Vega => "vega",
    BlockDiag => "blockdiag",
    ActDiag => "actdiag",
    SeqDiag => "seqdiag",
    NwDiag => "nwdiag",
    PacketDiag => "packetdiag",
    RackDiag => "rackdiag",
    Erd => "erd",
    Umlet => "umlet",
    Pikchr => "pikchr",
    Goat => "goat",
    ByteField => "bytefield",
    Dbml => "dbml",
    TikZ => "tikz",
    SvgBob => "svgbob",
    WireViz => "wireviz",
    Structurizr => "structurizr",
    Symbolator => "symbolator",
}

impl DiagramFormat {
    /// The path segment Kroki's HTTP API expects: `POST {base}/{slug}/{output}`.
    pub const fn kroki_slug(self) -> &'static str {
        self.0
    }
}

/// Unknown or companion-only format name.
#[derive(Debug)]
pub struct UnsupportedFormat(pub String);

impl std::fmt::Display for UnsupportedFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let supported = DiagramFormat::ALL
            .iter()
            .map(|d| d.kroki_slug())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "unsupported diagram format {:?}; kr0ki P0 supports: {supported}",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedFormat {}

impl FromStr for DiagramFormat {
    type Err = UnsupportedFormat;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let norm = s.trim().to_ascii_lowercase();
        DiagramFormat::ALL
            .iter()
            .copied()
            .find(|f| f.kroki_slug() == norm)
            .ok_or_else(|| UnsupportedFormat(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_slug_round_trips_through_from_str() {
        for &f in DiagramFormat::ALL {
            assert_eq!(f.kroki_slug().parse::<DiagramFormat>().unwrap(), f);
        }
    }

    #[test]
    fn from_str_is_case_and_whitespace_insensitive() {
        assert_eq!(
            "  PlantUML ".parse::<DiagramFormat>().unwrap(),
            DiagramFormat::PlantUml
        );
    }

    #[test]
    fn companion_only_and_unknown_formats_are_rejected() {
        for bad in [
            "mermaid",
            "excalidraw",
            "bpmn",
            "diagramsnet",
            "totally-made-up",
        ] {
            let err = bad.parse::<DiagramFormat>().unwrap_err();
            assert!(err.to_string().contains(bad));
        }
    }

    #[test]
    fn all_has_one_entry_per_declared_const_and_no_duplicate_slugs() {
        // The macro can't silently drop a const from ALL (there's only one list), but
        // it also can't stop two consts sharing a slug — that would make one
        // unreachable via FromStr. Guard it here instead.
        let mut slugs: Vec<&str> = DiagramFormat::ALL.iter().map(|f| f.kroki_slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(
            slugs.len(),
            before,
            "duplicate kroki_slug in DiagramFormat::ALL"
        );
        assert_eq!(DiagramFormat::ALL.len(), 26);
    }
}
