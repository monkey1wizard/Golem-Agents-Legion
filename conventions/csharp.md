# C# / .NET Conventions

Target: **.NET 10** / **C# 14**. Use file-scoped namespaces, global usings, CRLF on Windows.

---

## Naming (Microsoft Style)

| Category | Convention | Example |
| --- | --- | --- |
| Classes, Records, Structs | `PascalCase` | `OrderValidation` |
| Interfaces | `I` + `PascalCase` | `IMenuStorage` |
| Methods, Properties | `PascalCase` | `ValidateAsync()` |
| Private instance/static fields | `_camelCase` | `_logger`, `_jsonOptions` |
| Constants (`const`) | `PascalCase` | `MaxRetries` |
| Local variables, parameters | `camelCase` | `orderCount` |
| Enums | `PascalCase` | `OrderState.Pending` |
| Type parameters | `T` prefix | `TResult` |
| Domain acronyms | Full caps | `LLM`, `ASR`, `API` |

---

## Modern C# Syntax

Always prefer newest syntax. Selection criteria: reads closer to English, then more concise.

| Feature (C# ver) | Prefer | Avoid |
| --- | --- | --- |
| Null-conditional assignment (14) | `customer?.Order = GetOrder();` | `if (customer is not null) ...` |
| `field` keyword (14) | `set { field = value.Trim(); }` | Manual `_name` backing field |
| Extension members (14) | `extension(Order) { ... }` | `static class OrderExtensions` |
| Collection expressions (12) | `[1, 2, 3]` | `new int[] { 1, 2, 3 }` |
| Primary constructors (12) | `class Svc(IDep dep)` | Manual ctor + `_dep` field |
| Raw string literals (11) | `"""{ "key": "val" }"""` | `"{ \"key\": \"val\" }"` |
| Required members (11) | `required string Name` | Ctor parameter for mandatory props |
| Pattern matching (9+) | `is null`, `is not null` | `== null`, `!= null` |
| Target-typed `new` (9) | `List<Order> orders = new();` | `var orders = new List<Order>();` when type obvious |
| Switch expressions (8) | `x switch { 1 => "a", _ => "b" }` | `switch` statement |

### Syntax Decision Tree

```text
Store injected dependency?         → Primary constructor
Backing field with custom logic?   → field keyword (C# 14)
Observable property in ViewModel?  → [ObservableProperty] partial property
Initialize a collection?           → Collection expression
Check for null?                    → Pattern matching (is null / is not null)
Branch on a value?                 → Switch expression
```

---

## Primary Constructors

Always use. No manual backing fields for injected services.

```csharp
// ✅
public class OrderService(IRepository repository, ILogger<OrderService> logger)
{
    public async Task ProcessAsync() => await repository.SaveAsync();
}
```

## ViewModels (CommunityToolkit.Mvvm)

Inherit `ObservableObject`. Use `[ObservableProperty]` with partial property syntax (8.4+).

```csharp
public partial class OrderViewModel(ILLMService llmService) : ObservableObject
{
    [ObservableProperty]
    public partial bool IsRecording { get; set; }

    [RelayCommand]
    private async Task ToggleRecordingAsync() => await llmService.StartAsync();
}
```

## Async/Await

- Always `async/await`. **Never** `.Result` or `.Wait()` (causes deadlocks).
- Use Constructor Injection. Never service locator pattern.

## Documentation Comments

**Self-documenting names over comments.** Only write `/// <summary>` when the name alone cannot convey non-obvious behavior, side effects, or constraints. Use `//` only for "why", never for "what".

---

## Clean Architecture

### Layer Dependencies

```text
[ Presentation ]    [ Infrastructure ]
         │                  │
         └────────┬─────────┘
                  ▼
           [ Application ]
                  │
                  ▼
             [ Domain ]
```

- **Domain**: Zero dependencies (entities, value objects, domain services)
- **Application**: Depends only on Domain (use cases, interfaces, DTOs)
- **Infrastructure**: Implements Application interfaces (DB, API, file system)
- **Presentation**: Depends on Application (ViewModels, UI)

### Placement Decision Tree

```text
New class → What does it do?
    ├─ Pure business rule, entity, value object?        → Domain
    ├─ Orchestrates use cases, defines interfaces?      → Application
    ├─ Talks to external systems (DB, API, LLM)?       → Infrastructure
    └─ Drives UI (ViewModel, page logic)?               → Presentation
```

### Key Rules

- Dependencies point inward. Inner layers must not know about outer layers.
- **Never duplicate folder names across layers** (use `UseCases/`, `DomainServices/`, `ExternalServices/` instead of `Services/` everywhere).
- Register DI in composition root (entry point), not in inner layers.

### Recommended Folders

| Layer | Folders |
| --- | --- |
| Domain | Entities, Models, Common, Constants, Enums |
| Application | Interfaces, Services, Stubs |
| Infrastructure | Adapters, LLMProviders, Storage, ASR |
| Presentation | ViewModels, Resources |

---

## Blazor

### Component Structure

- `PascalCase` for component files (`OrderList.razor`)
- Simple components: `@code { }` in `.razor`. Complex: partial class (`.razor.cs`)
- `[Parameter]` must be `public`. Use `[EditorRequired]` for mandatory. Use `[SupplyParameterFromQuery]` for URL params.
- Global `@using`/`@inject` in `_Imports.razor`. Don't duplicate in individual files.

### Events and Lifecycle

- Use `@onclick="MethodName"` (delegate) over lambda when no args needed
- Always use `Async` lifecycle methods (`OnInitializedAsync`)
- Implement `IDisposable` for event subscription cleanup

### Styling

- **MudBlazor first**: Use MudBlazor components over raw HTML
- Bootstrap utility classes for layout/spacing
- Custom CSS via CSS Isolation (`.razor.css`) with `kebab-case` classes
- Avoid inline styles

### Error Handling

- Wrap major UI sections in `<ErrorBoundary>`
- Follow Result pattern for logic errors
