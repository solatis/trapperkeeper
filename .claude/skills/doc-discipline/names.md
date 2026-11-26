# Names

Read this before you name a type, a trait, a function, a field, a module or a
file, in code or in a plan.

A name says what sets its thing apart from the things beside it: what it
holds, how its contents are arranged, or what it does. A word that fits any
collection or any piece of code, such as `Values`, `Items`, `Elements`,
`Data`, `Info`, `Manager` or `handle`, is never a name on its own. These
pairs show the difference:

| Fits anything                               | Says what sets it apart |
| ------------------------------------------- | ----------------------- |
| `Data`, for jobs that wait for a worker     | `PendingJobs`           |
| `Items`, for the lines of an invoice        | `InvoiceLines`          |
| `Values`, for samples taken at a fixed rate | `FixedRateSamples`      |
| `Info`, for the display settings of a user  | `DisplayPreferences`    |
| `Manager`, for what retries failed uploads  | `UploadRetrier`         |
| `handle()`, for what records a paid invoice | `record_payment()`      |
