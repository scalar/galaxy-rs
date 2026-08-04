# Scalar Galaxy Rust API

Complete reference of every operation, grouped by resource. See [the README](./README.md) for usage and configuration.

## Contents

- [`Planets`](#planets)
  - [Get all planets](#get-all-planets)
  - [Create a planet](#create-a-planet)
  - [Get a planet](#get-a-planet)
  - [Update a planet](#update-a-planet)
  - [Delete a planet](#delete-a-planet)
  - [Upload an image to a planet](#upload-an-image-to-a-planet)
- [`CelestialBodies`](#celestialbodies)
  - [Create a celestial body](#create-a-celestial-body)
- [`Authentication`](#authentication)
  - [Create a user](#create-a-user)
  - [Get a token](#get-a-token)
  - [Get authenticated user](#get-authenticated-user)

## Setup

```rust
use scalar_galaxy::*;

let client = Galaxy::from_env()?;
```

## `Planets`

### Get all planets

It's easy to say you know them all, but do you really? Retrieve all the planets and check whether you missed one.

| Direction | Type |
| --- | --- |
| Response | [`PaginatedResource`](./src/models.rs) |

```rust
let response = client.planets().list_all_data().send().await?;
```

### Create a planet

Time to play god and create a new planet. What do you think? Ah, don't think too much. What could go wrong anyway?

| Direction | Type |
| --- | --- |
| Request | [`Planet`](./src/models.rs) |
| Response | [`Planet`](./src/models.rs) |

```rust
let response = client.planets().create().send().await?;
```

### Get a planet

You'll better learn a little bit more about the planets. It might come in handy once space travel is available for everyone.

| Direction | Type |
| --- | --- |
| Response | [`Planet`](./src/models.rs) |

```rust
let response = client.planets().retrieve(1).send().await?;
```

### Update a planet

Sometimes you make mistakes, that's fine. No worries, you can update all planets.

| Direction | Type |
| --- | --- |
| Request | [`Planet`](./src/models.rs) |
| Response | [`Planet`](./src/models.rs) |

```rust
let response = client.planets().update(1).send().await?;
```

### Delete a planet

This endpoint was used to delete planets. Unfortunately, that caused a lot of trouble for planets with life. So, this endpoint is now deprecated and should not be used anymore.

| Direction | Type |
| --- | --- |
| Response | `()` |

```rust
client.planets().delete(1).send().await?;
```

### Upload an image to a planet

Got a crazy good photo of a planet? Share it with the world!

| Direction | Type |
| --- | --- |
| Response | [`PlanetsUploadImageResponse`](./src/models.rs) |

```rust
let response = client.planets().upload_image(1).send().await?;
```

## `CelestialBodies`

### Create a celestial body

Stars, moons, comets, the occasional rogue asteroid — if it glows or drifts through the void, you can add it here.

| Direction | Type |
| --- | --- |
| Request | [`CelestialBody`](./src/models.rs) |
| Response | [`CelestialBody`](./src/models.rs) |

## `Authentication`

### Create a user

Time to create a user account, eh?

| Direction | Type |
| --- | --- |
| Request | [`AuthenticationCreateUserBody`](./src/models.rs) |
| Response | [`User`](./src/models.rs) |

```rust
let response = client.authentication().create_user().send().await?;
```

### Get a token

Yeah, this is the boring security stuff. Just get your super secret token and move on.

| Direction | Type |
| --- | --- |
| Request | [`Credentials`](./src/models.rs) |
| Response | [`Token`](./src/models.rs) |

```rust
let response = client.authentication().create_token().send().await?;
```

### Get authenticated user

Find yourself they say. That's what you can do here.

| Direction | Type |
| --- | --- |
| Response | [`User`](./src/models.rs) |

```rust
let response = client.authentication().list_me().send().await?;
```
