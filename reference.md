# ScalarGalaxy Rust SDK Reference

## Operations

### client.planets().list_all_data()

Get all planets

- HTTP: `GET /planets`
- Response body: `application/json`

### client.planets().create()

Create a planet

- HTTP: `POST /planets`
- Request body: `application/json`
- Response body: `application/json`
- Errors: `400, 403`

### client.planets().retrieve(…)

Get a planet

- HTTP: `GET /planets/{planetId}`
- Response body: `application/json`
- Errors: `404`

### client.planets().update(…)

Update a planet

- HTTP: `PUT /planets/{planetId}`
- Request body: `application/json`
- Response body: `application/json`
- Errors: `400, 403, 404`

### client.planets().delete(…)

Delete a planet

- HTTP: `DELETE /planets/{planetId}`
- Errors: `404`

### client.planets().upload_image(…)

Upload an image to a planet

- HTTP: `POST /planets/{planetId}/image`
- Request body: `multipart/form-data`
- Response body: `application/json`
- Errors: `400, 403, 404`

### client.celestial_bodies().create(…)

Create a celestial body

- HTTP: `POST /celestial-bodies`
- Request body: `application/json`
- Response body: `application/json`

### client.authentication().create_user()

Create a user

- HTTP: `POST /user/signup`
- Request body: `application/json`
- Response body: `application/json`
- Errors: `400, 401, 403, 409, 422`

### client.authentication().create_token()

Get a token

- HTTP: `POST /auth/token`
- Request body: `application/json`
- Response body: `application/json`
- Errors: `400, 401, 403, 429`

### client.authentication().list_me()

Get authenticated user

- HTTP: `GET /me`
- Response body: `application/json`
- Errors: `401, 403`

## Models

- `User`
- `Credentials`
- `Token`
- `CelestialBody`
- `Planet`
- `Satellite`
- `PaginatedResource`
