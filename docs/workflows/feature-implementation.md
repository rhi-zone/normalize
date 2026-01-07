# Feature Implementation Workflow

Adding new functionality to a codebase - from requirement to working, tested, reviewed code.

## Trigger

- User request / feature request
- Product requirement
- Technical improvement
- Integration need
- Self-identified enhancement

## Goal

- Working feature that meets requirements
- Tests that verify behavior
- Documentation for users/maintainers
- Code that fits existing architecture
- Clean commit history for review

## Prerequisites

- Clear understanding of requirements
- Access to codebase
- Knowledge of existing architecture
- Test infrastructure
- Review process

## Why Feature Implementation Is Hard

1. **Requirements clarity**: What exactly should it do?
2. **Architecture fit**: Where does it belong?
3. **Scope creep**: "While we're at it..."
4. **Edge cases**: What about X?
5. **Testing**: How do we verify it works?
6. **Integration**: How does it interact with existing code?

## Core Strategy: Understand → Design → Implement → Verify

```
┌─────────────────────────────────────────────────────────┐
│                    UNDERSTAND                            │
│  Requirements, constraints, existing patterns           │
└─────────────────────┬───────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────┐
│                      DESIGN                              │
│  Where it goes, how it works, API surface               │
└─────────────────────┬───────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────┐
│                    IMPLEMENT                             │
│  Write code, tests, documentation                       │
└─────────────────────┬───────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────┐
│                      VERIFY                              │
│  Tests pass, requirements met, code reviewed            │
└─────────────────────────────────────────────────────────┘
```

## Phase 1: Understand

### Clarify Requirements

```markdown
## Feature: User Export

### What the user asked for
"I want to export my data"

### Clarifying questions
- What data? (profile, posts, comments, all?)
- What format? (JSON, CSV, PDF?)
- Where? (download, email, cloud storage?)
- How often? (one-time, recurring?)
- Who can export? (own data only, admin can export any?)

### Refined requirement
Users can export their own profile and posts as JSON,
downloaded as a .zip file, on-demand from settings page.
```

### Understand Existing Patterns

```bash
# How does the codebase handle similar features?

# Find existing export/download functionality
grep -rn "export\|download\|generate.*file" src/

# Find how settings page adds features
ls src/settings/
cat src/settings/README.md

# Find how background jobs are handled (if export is async)
grep -rn "queue\|job\|worker\|async" src/
```

### Identify Constraints

```markdown
## Constraints

### Technical
- Must complete in < 30 seconds (or background job)
- Output file < 100MB
- No external dependencies for export format

### Business
- Must be GDPR compliant (export all personal data)
- Audit log of exports required
- Rate limit: 1 export per hour per user

### Existing patterns
- Settings page uses React components in src/settings/
- API endpoints in src/api/v2/
- Background jobs via Redis queue
```

## Phase 2: Design

### Choose Location

```markdown
## Architecture Decision

### Options considered
1. Inline in API handler (simple, but blocks request)
2. Background job (async, but adds complexity)
3. Streaming response (progressive, but complex error handling)

### Decision: Background job
- Export can take > 30s for users with lots of data
- User gets notification when ready
- Follows existing pattern for report generation

### File locations
- API endpoint: src/api/v2/exports.py
- Job handler: src/jobs/export_user_data.py
- Frontend: src/settings/components/ExportData.tsx
```

### Define API Surface

```python
# API Design

# Trigger export
POST /api/v2/users/{id}/exports
Request: { "format": "json", "include": ["profile", "posts"] }
Response: { "export_id": "abc123", "status": "pending" }

# Check status
GET /api/v2/users/{id}/exports/{export_id}
Response: { "status": "completed", "download_url": "...", "expires_at": "..." }

# List exports
GET /api/v2/users/{id}/exports
Response: { "exports": [...] }
```

### Plan Tests

```markdown
## Test Plan

### Unit tests
- Export job correctly gathers user data
- JSON serialization handles edge cases (unicode, dates)
- Zip creation works with various file sizes

### Integration tests
- API endpoint triggers job correctly
- Status updates as job progresses
- Download URL works and expires correctly

### Edge cases
- User with no posts
- User with 10,000 posts (large export)
- Export while user is being deleted
- Concurrent export requests
```

## Phase 3: Implement

### Start with Tests (TDD approach)

```python
# Write failing test first
def test_export_creates_valid_json():
    user = create_test_user(posts=5)

    result = export_user_data(user.id, format='json')

    assert result['profile']['email'] == user.email
    assert len(result['posts']) == 5

def test_export_respects_rate_limit():
    user = create_test_user()
    export_user_data(user.id)  # First export

    with pytest.raises(RateLimitError):
        export_user_data(user.id)  # Second within hour
```

### Implement Core Logic

```python
# src/jobs/export_user_data.py

def export_user_data(user_id: int, format: str, include: list[str]) -> ExportResult:
    """Export user data to specified format.

    Args:
        user_id: User to export
        format: Output format ('json')
        include: Data types to include ('profile', 'posts')

    Returns:
        ExportResult with file path and metadata
    """
    user = User.query.get_or_404(user_id)

    # Check rate limit
    if recent_export_exists(user_id):
        raise RateLimitError("Export limit: 1 per hour")

    # Gather data
    data = {}
    if 'profile' in include:
        data['profile'] = serialize_profile(user)
    if 'posts' in include:
        data['posts'] = [serialize_post(p) for p in user.posts]

    # Create export file
    if format == 'json':
        file_path = create_json_export(user_id, data)
    else:
        raise ValueError(f"Unsupported format: {format}")

    # Record export
    export = Export.create(user_id=user_id, file_path=file_path)

    return ExportResult(
        export_id=export.id,
        file_path=file_path,
        size=os.path.getsize(file_path),
    )
```

### Implement API Endpoint

```python
# src/api/v2/exports.py

@router.post('/users/{user_id}/exports')
@login_required
def create_export(user_id: int, request: ExportRequest):
    # Authorization
    if current_user.id != user_id and not current_user.is_admin:
        raise Forbidden()

    # Validate request
    if request.format not in ['json']:
        raise BadRequest(f"Unsupported format: {request.format}")

    # Queue job
    job = export_queue.enqueue(
        export_user_data,
        user_id=user_id,
        format=request.format,
        include=request.include,
    )

    return {'export_id': job.id, 'status': 'pending'}
```

### Implement Frontend

```typescript
// src/settings/components/ExportData.tsx

export function ExportData() {
  const [status, setStatus] = useState<'idle' | 'pending' | 'ready'>('idle');
  const [downloadUrl, setDownloadUrl] = useState<string | null>(null);

  async function handleExport() {
    setStatus('pending');
    const { export_id } = await api.post('/users/me/exports', {
      format: 'json',
      include: ['profile', 'posts'],
    });

    // Poll for completion
    const result = await pollUntilReady(export_id);
    setDownloadUrl(result.download_url);
    setStatus('ready');
  }

  return (
    <div>
      <h2>Export Your Data</h2>
      {status === 'idle' && (
        <button onClick={handleExport}>Export as JSON</button>
      )}
      {status === 'pending' && <Spinner />}
      {status === 'ready' && (
        <a href={downloadUrl}>Download Export</a>
      )}
    </div>
  );
}
```

### Add Documentation

```markdown
# User Data Export

Export your personal data in JSON format.

## How to export

1. Go to Settings > Privacy
2. Click "Export Your Data"
3. Wait for export to complete (may take a few minutes)
4. Download the .zip file

## What's included

- Profile information (name, email, settings)
- All your posts and comments
- Account metadata

## Limitations

- One export per hour
- Export files are available for 24 hours
- Maximum export size: 100MB
```

## Phase 4: Verify

### Run Tests

```bash
# Unit tests
pytest tests/unit/test_export.py -v

# Integration tests
pytest tests/integration/test_export_api.py -v

# Full test suite
pytest
```

### Manual Testing

```markdown
## Manual Test Checklist

- [ ] Export triggers from UI
- [ ] Progress indicator shows
- [ ] Download link appears when ready
- [ ] Downloaded file contains expected data
- [ ] Rate limit prevents rapid exports
- [ ] Large export (1000+ posts) completes
- [ ] Empty export (new user) works
- [ ] Error handling (job failure) shows user-friendly message
```

### Code Review Prep

```markdown
## PR Description

### What
Adds user data export feature (JSON format).

### Why
GDPR compliance requires users to export their data.

### How
- Background job gathers and serializes user data
- API endpoint triggers job, returns status
- Frontend component in settings page

### Testing
- Unit tests for serialization and job logic
- Integration tests for API endpoints
- Manual testing checklist completed

### Screenshots
[Settings page with export button]
[Export in progress]
[Download ready]
```

## Implementation Patterns

### Incremental Development

```
1. Hardcoded prototype (prove it works)
2. Extract configuration (make it flexible)
3. Add error handling (make it robust)
4. Add tests (make it reliable)
5. Add documentation (make it usable)
```

### Feature Flags

```python
# Gate incomplete features
if feature_flags.enabled('user_export', user_id):
    show_export_button()
```

### Backward Compatibility

```python
# Old API still works
@router.get('/api/v1/export')  # Deprecated
def old_export():
    return redirect('/api/v2/users/me/exports')

# New API
@router.post('/api/v2/users/{id}/exports')
def new_export(id: int):
    ...
```

## Common Mistakes

| Mistake | Why It's Bad | Prevention |
|---------|--------------|------------|
| Implementing before understanding | Build wrong thing | Clarify requirements first |
| Ignoring existing patterns | Inconsistent codebase | Study existing code |
| No tests | Regression risk | TDD or test-after |
| Big bang commit | Hard to review | Incremental commits |
| Scope creep | Never ships | Strict scope, defer extras |

## LLM-Assisted Implementation

### Requirement Clarification

```
The user wants "export functionality". Help me clarify:

Current system: Social media app with profiles and posts
User said: "I want to export my data"

Generate clarifying questions to understand:
1. What data to export
2. What format
3. Where/how to deliver
4. Who can export what
5. Any constraints (size, frequency)
```

### Design Review

```
Review this API design for a user export feature:

POST /api/exports
{ "user_id": 123, "format": "json" }

Identify:
1. Security issues
2. API design issues
3. Missing considerations
4. Suggestions for improvement
```

### Code Generation

```
Implement a background job for user data export:

Requirements:
- Gather user profile and posts
- Serialize to JSON
- Create zip file
- Store with expiring URL
- Rate limit: 1/hour/user

Existing patterns:
- Jobs use Redis queue
- Files stored in S3
- Models: User, Post, Export

Generate the job handler code.
```

## Failure Modes

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Requirement misunderstood | User feedback, review | Iterate on design |
| Doesn't fit architecture | Code review | Refactor to match |
| Missing edge cases | Bug reports, tests | Add tests, fix |
| Performance issues | Load testing | Optimize, async |

## See Also

- [Bug Fix](bug-fix.md) - Fixing issues in features
- [Code Review](code-review.md) - Getting features reviewed
- [Refactoring](refactoring.md) - Improving feature code later

