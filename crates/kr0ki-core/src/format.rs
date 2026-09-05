//! The Kroki-family diagram formats kr0ki's P0 loop accepts.
//!
//! Deliberately *only* the formats a stock Kroki container renders with no headless-
//! Chromium companion (PRD-KR0KI-001 NFR3). `mermaid`, `bpmn`, `excalidraw` are left
//! out on purpose — they need the companion services this P0 does not run.

use std::str::FromStr;

/// A diagram source language kr0ki can hand to a Kroki backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagramFormat {
    PlantUml,
    C4PlantUml,
    GraphViz,
    D2,
    VegaLite,
    Ditaa,
    Nomnoml,
    WaveDrom,
}

impl DiagramFormat {
    /// The path segment Kroki's HTTP API expects: `POST {base}/{slug}/{output}`.
    pub const fn kroki_slug(self) -> &'static str {
        match self {
            Self::PlantUml => "plantuml",
            Self::C4PlantUml => "c4plantuml",
            Self::GraphViz => "graphviz",
            Self::D2 => "d2",
            Self::VegaLite => "vegalite",
            Self::Ditaa => "ditaa",
            Self::Nomnoml => "nomnoml",
            Self::WaveDrom => "wavedrom",
        }
    }

    /// Every variant, for callers that enumerate the supported set (e.g. a `/formats`
    /// endpoint). Kept in sync with the enum by the exhaustiveness test below.
    pub const ALL: &'static [DiagramFormat] = &[
        Self::PlantUml,
        Self::C4PlantUml,
        Self::GraphViz,
        Self::D2,
        Self::VegaLite,
        Self::Ditaa,
        Self::Nomnoml,
        Self::WaveDrom,
    ];
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
        for bad in ["mermaid", "excalidraw", "bpmn", "totally-made-up"] {
            let err = bad.parse::<DiagramFormat>().unwrap_err();
            assert!(err.to_string().contains(bad));
        }
    }

    #[test]
    fn all_is_exhaustive() {
        // If a variant is added without extending ALL, this match stops compiling.
        for &f in DiagramFormat::ALL {
            match f {
                DiagramFormat::PlantUml
                | DiagramFormat::C4PlantUml
                | DiagramFormat::GraphViz
                | DiagramFormat::D2
                | DiagramFormat::VegaLite
                | DiagramFormat::Ditaa
                | DiagramFormat::Nomnoml
                | DiagramFormat::WaveDrom => {}
            }
        }
        assert_eq!(DiagramFormat::ALL.len(), 8);
    }
}
