"""The small core vocabulary for ad hoc document knowledge (docling-graph extraction template).

Deliberately generic: no domain classes. `Entity`/`Concept` are things the document talks about, a `Claim` is something it says
about them. Provenance is not modelled here: docling-graph records source chunk, page and bbox on every node, and kr0ki maps
that to PROV-O. Per the docling-graph template rules, validators normalize and never reject, and only identity fields are required.
"""
from typing import Any, List, Optional

from pydantic import BaseModel, ConfigDict, Field


def edge(label: str, reference: bool = False, **kwargs: Any) -> Any:
    """Declare a field as a graph edge (docling-graph reads `edge_label` from json_schema_extra).

    `reference=True` marks an identity-only link to an entity described in full elsewhere (`graph_reference`): the parent's own
    fill call supplies it, which is what keeps edges from coming back empty on a small local model.
    """
    extra = dict(kwargs.pop("json_schema_extra", {}) or {})
    extra["edge_label"] = label
    if reference:
        extra["graph_reference"] = True
    return Field(json_schema_extra=extra, **kwargs)


class Concept(BaseModel):
    """An idea, topic, term or category the document discusses (e.g. "ownership", "garbage collection")."""

    model_config = ConfigDict(graph_id_fields=["name"])
    name: str = Field(description="Short canonical name of the concept, as the document names it.")
    definition: Optional[str] = Field(default=None, description="One sentence definition, only if the document gives one.")
    broader: List["Concept"] = edge("BROADER", reference=True, default_factory=list, description="More general concepts this one is a kind of.")


class Entity(BaseModel):
    """A named thing: a person, organisation, product, system, tool, standard, place or document."""

    model_config = ConfigDict(graph_id_fields=["name"])
    name: str = Field(description="The name exactly as written in the document.")
    kind: Optional[str] = Field(default=None, description="Free-text type, e.g. 'language', 'person', 'standard'.")
    related_to: List["Entity"] = edge("RELATED_TO", reference=True, default_factory=list, description="Other entities this one is explicitly related to.")


class Claim(BaseModel):
    """A single assertion the document makes, stated in one sentence and kept close to the source wording."""

    model_config = ConfigDict(graph_id_fields=["statement"])
    statement: str = Field(description="The assertion as one self-contained sentence.")
    mentions: List[Entity] = edge("MENTIONS", reference=True, default_factory=list, description="Entities the claim is about.")
    about: List[Concept] = edge("ABOUT", reference=True, default_factory=list, description="Concepts the claim is about.")


class DocumentKnowledge(BaseModel):
    """Root: everything worth keeping from the document as entities, concepts and claims.

    The root MUST be an entity (the default). docling-graph starts its edge walk at the root and returns at once when the root is
    a component (`is_entity=False`), so every edge below it is silently dropped: that gave "N nodes, 0 edges" with a model output
    that did contain the relations (verified 2026-10-07: same output, 36 nodes / 0 edges vs 37 nodes / 67 edges).
    """

    model_config = ConfigDict(graph_id_fields=["title"])
    title: str = Field(description="The document's title; if it has none, a short descriptive title taken from its first heading.")
    claims: List[Claim] = edge("HAS_CLAIM", default_factory=list, description="The assertions the document makes.")
    entities: List[Entity] = edge("HAS_ENTITY", default_factory=list, description="Named things the document mentions.")
    concepts: List[Concept] = edge("HAS_CONCEPT", default_factory=list, description="Ideas and topics the document discusses.")
