"""Tests for the parallel processing module."""

import asyncio
from pathlib import Path

import pytest

from moss.parallel import (
    AnalysisResult,
    BatchStats,
    ParallelAnalyzer,
    parallel_analyze,
    parallel_map,
    sync_parallel_analyze,
)


class TestAnalysisResult:
    """Tests for AnalysisResult."""

    def test_create_success_result(self):
        result = AnalysisResult(
            path=Path("test.py"),
            result="success",
            duration_ms=100.0,
        )

        assert result.path == Path("test.py")
        assert result.result == "success"
        assert result.error is None
        assert result.success is True

    def test_create_error_result(self):
        result = AnalysisResult(
            path=Path("test.py"),
            error="parse error",
            duration_ms=50.0,
        )

        assert result.path == Path("test.py")
        assert result.result is None
        assert result.error == "parse error"
        assert result.success is False


class TestBatchStats:
    """Tests for BatchStats."""

    def test_success_rate_zero_total(self):
        stats = BatchStats()
        assert stats.success_rate == 0.0

    def test_success_rate_calculation(self):
        stats = BatchStats(total=10, completed=10, failed=2)
        assert stats.success_rate == 0.8

    def test_success_rate_all_success(self):
        stats = BatchStats(total=5, completed=5, failed=0)
        assert stats.success_rate == 1.0

    def test_to_dict(self):
        stats = BatchStats(total=10, completed=10, failed=1, duration_ms=1000.0)
        result = stats.to_dict()

        assert result["total"] == 10
        assert result["completed"] == 10
        assert result["failed"] == 1
        assert result["duration_ms"] == 1000.0
        assert "success_rate" in result


class TestParallelAnalyzer:
    """Tests for ParallelAnalyzer."""

    def test_create_analyzer(self):
        analyzer = ParallelAnalyzer(max_workers=4)

        assert analyzer.max_workers == 4
        assert analyzer.use_processes is False
        assert analyzer.batch_size == 100

    def test_create_with_processes(self):
        analyzer = ParallelAnalyzer(use_processes=True)
        assert analyzer.use_processes is True

    @pytest.mark.asyncio
    async def test_context_manager(self):
        async with ParallelAnalyzer(max_workers=2) as analyzer:
            assert analyzer._executor is not None
            assert analyzer._semaphore is not None

        assert analyzer._executor is None
        assert analyzer._semaphore is None

    @pytest.mark.asyncio
    async def test_analyze_file_sync_func(self, tmp_path: Path):
        path = tmp_path / "test.py"
        path.write_text("x = 1")

        def analyze(p: Path) -> int:
            return len(p.read_text())

        async with ParallelAnalyzer() as analyzer:
            result = await analyzer.analyze_file(path, analyze)

        assert result.success is True
        assert result.result == 5
        assert result.duration_ms > 0

    @pytest.mark.asyncio
    async def test_analyze_file_async_func(self, tmp_path: Path):
        path = tmp_path / "test.py"
        path.write_text("x = 1")

        async def analyze(p: Path) -> int:
            return len(p.read_text())

        async with ParallelAnalyzer() as analyzer:
            result = await analyzer.analyze_file(path, analyze)

        assert result.success is True
        assert result.result == 5

    @pytest.mark.asyncio
    async def test_analyze_file_error(self, tmp_path: Path):
        path = tmp_path / "test.py"
        path.write_text("x = 1")

        def analyze(p: Path) -> int:
            raise ValueError("test error")

        async with ParallelAnalyzer() as analyzer:
            result = await analyzer.analyze_file(path, analyze)

        assert result.success is False
        assert "test error" in result.error

    @pytest.mark.asyncio
    async def test_analyze_files(self, tmp_path: Path):
        # Create test files
        files = []
        for i in range(5):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        def analyze(p: Path) -> str:
            return p.read_text()

        async with ParallelAnalyzer(max_workers=2) as analyzer:
            results = []
            async for result in analyzer.analyze_files(files, analyze):
                results.append(result)

        assert len(results) == 5
        assert all(r.success for r in results)

    @pytest.mark.asyncio
    async def test_analyze_files_with_progress(self, tmp_path: Path):
        files = []
        for i in range(3):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        progress_calls = []

        def on_progress(completed: int, total: int):
            progress_calls.append((completed, total))

        def analyze(p: Path) -> str:
            return p.read_text()

        async with ParallelAnalyzer() as analyzer:
            results = []
            async for result in analyzer.analyze_files(files, analyze, on_progress):
                results.append(result)

        assert len(progress_calls) == 3
        assert progress_calls[-1] == (3, 3)

    @pytest.mark.asyncio
    async def test_analyze_all(self, tmp_path: Path):
        files = []
        for i in range(4):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        def analyze(p: Path) -> str:
            return p.read_text()

        async with ParallelAnalyzer() as analyzer:
            results, stats = await analyzer.analyze_all(files, analyze)

        assert len(results) == 4
        assert stats.total == 4
        assert stats.completed == 4
        assert stats.failed == 0
        assert stats.duration_ms > 0

    @pytest.mark.asyncio
    async def test_analyze_all_with_errors(self, tmp_path: Path):
        files = []
        for i in range(3):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        def analyze(p: Path) -> str:
            if "test1" in str(p):
                raise ValueError("test error")
            return p.read_text()

        async with ParallelAnalyzer() as analyzer:
            results, stats = await analyzer.analyze_all(files, analyze)

        assert len(results) == 3
        assert stats.failed == 1
        assert stats.success_rate == pytest.approx(2 / 3)


class TestParallelAnalyze:
    """Tests for the parallel_analyze convenience function."""

    @pytest.mark.asyncio
    async def test_parallel_analyze(self, tmp_path: Path):
        files = []
        for i in range(3):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        def analyze(p: Path) -> int:
            return len(p.read_text())

        results = await parallel_analyze(files, analyze, max_workers=2)

        assert len(results) == 3
        assert all(r.success for r in results)

    @pytest.mark.asyncio
    async def test_parallel_analyze_empty(self):
        results = await parallel_analyze([], lambda p: None)
        assert results == []


class TestParallelMap:
    """Tests for the parallel_map function."""

    @pytest.mark.asyncio
    async def test_parallel_map_sync(self):
        items = [1, 2, 3, 4, 5]

        def square(x: int) -> int:
            return x * x

        results = await parallel_map(items, square, max_workers=2)

        assert results == [1, 4, 9, 16, 25]

    @pytest.mark.asyncio
    async def test_parallel_map_async(self):
        items = [1, 2, 3]

        async def double(x: int) -> int:
            await asyncio.sleep(0.01)
            return x * 2

        results = await parallel_map(items, double, max_workers=2)

        assert results == [2, 4, 6]

    @pytest.mark.asyncio
    async def test_parallel_map_preserves_order(self):
        items = list(range(10))

        async def identity(x: int) -> int:
            await asyncio.sleep(0.01 * (10 - x))  # Reverse delay
            return x

        results = await parallel_map(items, identity, max_workers=4)

        assert results == items


class TestSyncParallelAnalyze:
    """Tests for the sync_parallel_analyze function."""

    def test_sync_parallel_analyze(self, tmp_path: Path):
        files = []
        for i in range(3):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        def analyze(p: Path) -> str:
            return p.read_text()

        results = sync_parallel_analyze(files, analyze, max_workers=2)

        assert len(results) == 3
        # Results may not be in order due to concurrent execution
        successful = [r for r in results if r.success]
        assert len(successful) == 3

    def test_sync_parallel_analyze_with_error(self, tmp_path: Path):
        files = []
        for i in range(3):
            path = tmp_path / f"test{i}.py"
            path.write_text(f"x = {i}")
            files.append(path)

        def analyze(p: Path) -> str:
            if "test1" in str(p):
                raise ValueError("test error")
            return p.read_text()

        results = sync_parallel_analyze(files, analyze)

        errors = [r for r in results if not r.success]
        assert len(errors) == 1
        assert "test error" in errors[0].error

    def test_sync_parallel_analyze_empty(self):
        results = sync_parallel_analyze([], lambda p: None)
        assert results == []
