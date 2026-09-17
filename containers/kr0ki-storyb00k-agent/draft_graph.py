"""A disposable, session-scoped RDF draft. It never writes to the model server."""

import uuid

import rdflib

KR0KI_NS = rdflib.Namespace("http://kr0ki.promptexecution.com/ontology#")


class DraftGraph:
    def __init__(self):
        self._graph = rdflib.Graph()
        self._pending = {}

    def propose(self, subject, predicate, obj):
        proposal_id = str(uuid.uuid4())
        self._pending[proposal_id] = (subject, predicate, obj)
        return proposal_id

    def apply(self, proposal_id):
        subject, predicate, obj = self._pending.pop(proposal_id)
        self._graph.add((KR0KI_NS[subject], KR0KI_NS[predicate], rdflib.Literal(obj)))

    def decline(self, proposal_id):
        self._pending.pop(proposal_id)

    def pending_proposals(self):
        return [{"id": proposal_id, "subject": subject, "predicate": predicate, "object": obj}
                for proposal_id, (subject, predicate, obj) in self._pending.items()]

    def as_turtle(self):
        return self._graph.serialize(format="turtle")
