"""Tests for Vector Store abstraction."""

import pytest

from moss.vector_store import (
    ChromaVectorStore,
    InMemoryVectorStore,
    SearchResult,
    VectorStore,
    create_vector_store,
    document_hash,
)


class TestSearchResult:
    """Tests for SearchResult dataclass."""

    def test_create_result(self):
        result = SearchResult(
            id="doc1",
            score=0.95,
            metadata={"type": "code"},
            document="def hello(): pass",
        )

        assert result.id == "doc1"
        assert result.score == 0.95
        assert result.metadata["type"] == "code"
        assert result.document == "def hello(): pass"

    def test_default_values(self):
        result = SearchResult(id="doc1", score=0.5)

        assert result.metadata == {}
        assert result.document is None


class TestInMemoryVectorStore:
    """Tests for InMemoryVectorStore."""

    @pytest.fixture
    def store(self) -> InMemoryVectorStore:
        return InMemoryVectorStore()

    async def test_add_and_get(self, store: InMemoryVectorStore):
        await store.add("doc1", "Python function", {"type": "code"})

        result = await store.get("doc1")

        assert result is not None
        assert result.id == "doc1"
        assert result.document == "Python function"
        assert result.metadata["type"] == "code"

    async def test_get_nonexistent(self, store: InMemoryVectorStore):
        result = await store.get("nonexistent")
        assert result is None

    async def test_search(self, store: InMemoryVectorStore):
        await store.add("doc1", "Python parsing function", {"type": "code"})
        await store.add("doc2", "JavaScript component", {"type": "code"})
        await store.add("doc3", "Python testing framework", {"type": "test"})

        results = await store.search("Python function")

        assert len(results) >= 1
        # Python docs should score higher
        python_results = [r for r in results if "Python" in (r.document or "")]
        assert len(python_results) >= 1

    async def test_search_with_filter(self, store: InMemoryVectorStore):
        await store.add("doc1", "Python code", {"type": "code"})
        await store.add("doc2", "Python test", {"type": "test"})

        results = await store.search("Python", filter={"type": "test"})

        assert len(results) == 1
        assert results[0].id == "doc2"

    async def test_search_with_limit(self, store: InMemoryVectorStore):
        for i in range(10):
            await store.add(f"doc{i}", f"Document about Python {i}", {})

        results = await store.search("Python", limit=3)

        assert len(results) == 3

    async def test_delete(self, store: InMemoryVectorStore):
        await store.add("doc1", "Test document", {})

        assert await store.delete("doc1")
        assert await store.get("doc1") is None
        assert not await store.delete("doc1")  # Already deleted

    async def test_count(self, store: InMemoryVectorStore):
        assert await store.count() == 0

        await store.add("doc1", "First", {})
        await store.add("doc2", "Second", {})

        assert await store.count() == 2

    async def test_clear(self, store: InMemoryVectorStore):
        await store.add("doc1", "First", {})
        await store.add("doc2", "Second", {})

        await store.clear()

        assert await store.count() == 0

    async def test_add_batch(self, store: InMemoryVectorStore):
        await store.add_batch(
            ids=["doc1", "doc2", "doc3"],
            documents=["First doc", "Second doc", "Third doc"],
            metadatas=[{"n": 1}, {"n": 2}, {"n": 3}],
        )

        assert await store.count() == 3
        result = await store.get("doc2")
        assert result is not None
        assert result.metadata["n"] == 2

    async def test_protocol_compliance(self, store: InMemoryVectorStore):
        """Verify InMemoryVectorStore satisfies VectorStore protocol."""
        assert isinstance(store, VectorStore)


class TestChromaVectorStore:
    """Tests for ChromaVectorStore."""

    @pytest.fixture
    def store(self) -> ChromaVectorStore:
        # Use in-memory ChromaDB for testing
        return ChromaVectorStore(collection_name="test_collection")

    def test_lazy_initialization(self):
        """ChromaDB should not initialize until first use."""
        store = ChromaVectorStore(collection_name="lazy_test")
        assert store._client is None
        assert store._collection is None

    async def test_add_and_get(self, store: ChromaVectorStore):
        pytest.importorskip("chromadb")

        await store.add("doc1", "Python function", {"type": "code"})
        result = await store.get("doc1")

        assert result is not None
        assert result.id == "doc1"
        assert result.document == "Python function"

    async def test_search(self, store: ChromaVectorStore):
        pytest.importorskip("chromadb")

        await store.add("doc1", "Python machine learning", {"type": "code"})
        await store.add("doc2", "JavaScript frontend", {"type": "code"})

        results = await store.search("Python AI", limit=5)

        assert len(results) >= 1
        # Python doc should be most relevant
        assert results[0].id == "doc1"

    async def test_delete(self, store: ChromaVectorStore):
        pytest.importorskip("chromadb")

        await store.add("doc1", "Test document", {})
        assert await store.delete("doc1")
        assert await store.get("doc1") is None

    async def test_count(self, store: ChromaVectorStore):
        pytest.importorskip("chromadb")

        await store.add("doc1", "First", {})
        await store.add("doc2", "Second", {})

        assert await store.count() == 2

    async def test_protocol_compliance(self, store: ChromaVectorStore):
        """Verify ChromaVectorStore satisfies VectorStore protocol."""
        assert isinstance(store, VectorStore)


class TestCreateVectorStore:
    """Tests for create_vector_store factory."""

    def test_create_memory_store(self):
        store = create_vector_store("memory")
        assert isinstance(store, InMemoryVectorStore)

    def test_create_chroma_store(self):
        store = create_vector_store("chroma", collection_name="test")
        assert isinstance(store, ChromaVectorStore)

    def test_unknown_backend(self):
        with pytest.raises(ValueError, match="Unknown backend"):
            create_vector_store("unknown")


class TestDocumentHash:
    """Tests for document_hash function."""

    def test_generates_hash(self):
        hash1 = document_hash("Hello world")
        assert len(hash1) == 8
        assert hash1.isalnum()

    def test_deterministic(self):
        hash1 = document_hash("Same content")
        hash2 = document_hash("Same content")
        assert hash1 == hash2

    def test_different_content(self):
        hash1 = document_hash("Content A")
        hash2 = document_hash("Content B")
        assert hash1 != hash2
