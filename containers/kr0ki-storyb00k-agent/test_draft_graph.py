import unittest

from draft_graph import DraftGraph


class DraftGraphTest(unittest.TestCase):
    def test_proposal_is_only_materialized_after_approval(self):
        draft = DraftGraph()
        proposal = draft.propose("elem-1", "name", "Engine v2")
        self.assertNotIn("Engine v2", draft.as_turtle())
        draft.apply(proposal)
        self.assertIn("Engine v2", draft.as_turtle())

    def test_declined_proposal_is_never_materialized(self):
        draft = DraftGraph()
        proposal = draft.propose("elem-1", "name", "Engine v2")
        draft.decline(proposal)
        self.assertNotIn("Engine v2", draft.as_turtle())

    def test_unknown_proposal_raises(self):
        with self.assertRaises(KeyError):
            DraftGraph().apply("unknown")

    def test_pending_only_lists_unresolved_proposals(self):
        draft = DraftGraph()
        applied = draft.propose("elem-1", "name", "Engine v2")
        pending = draft.propose("elem-2", "name", "Wing")
        draft.apply(applied)
        self.assertEqual(draft.pending_proposals()[0]["id"], pending)
