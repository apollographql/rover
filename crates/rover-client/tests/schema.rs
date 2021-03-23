use graphql_client::Response;
use rover_client::introspection::Schema;
use std::convert::TryFrom;
use std::fs::File;

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use rover_client::query::graph::introspect;

    pub type IntrospectionResult = introspect::introspection_query::ResponseData;
    #[test]
    fn it_builds_simple_schema() {
        let file = File::open("tests/fixtures/simple.json").unwrap();
        let res: Response<IntrospectionResult> = serde_json::from_reader(file).unwrap();

        let data = res.data.unwrap();
        let schema = Schema::try_from(data).unwrap();
        assert_eq!(
            schema.encode(),
            indoc! { r#"
        directive @cacheControl on FIELD_DEFINITION | OBJECT | INTERFACE
        """Exposes a URL that specifies the behaviour of this scalar."""
        directive @specifiedBy on SCALAR
        type Query {
          """A simple type for getting started!"""
          hello: String
          cats(cat: [String]! = ["Nori"]): [String]!
        }
        input BooleanQueryOperatorInput {
          eq: Boolean
          ne: Boolean
          in: [Boolean]
          nin: [Boolean]
        }
        enum CacheControlScope {
          PUBLIC
          PRIVATE
        }
        """The `Upload` scalar type represents a file upload."""
        scalar Upload
    "#}
        )
    }

    #[test]
    fn it_builds_swapi_schema() {
        let file = File::open("tests/fixtures/swapi.json").unwrap();
        let res: Response<IntrospectionResult> = serde_json::from_reader(file).unwrap();

        let data = res.data.unwrap();
        let schema = Schema::try_from(data).unwrap();
        assert_eq!(
            schema.encode(),
            indoc! { r#"
        schema {
          query: Root
        }
        type Root {
          allFilms(after: String, first: Int, before: String, last: Int): FilmsConnection
          film(id: ID, filmID: ID): Film
          allPeople(after: String, first: Int, before: String, last: Int): PeopleConnection
          person(id: ID, personID: ID): Person
          allPlanets(after: String, first: Int, before: String, last: Int): PlanetsConnection
          planet(id: ID, planetID: ID): Planet
          allSpecies(after: String, first: Int, before: String, last: Int): SpeciesConnection
          species(id: ID, speciesID: ID): Species
          allStarships(after: String, first: Int, before: String, last: Int): StarshipsConnection
          starship(id: ID, starshipID: ID): Starship
          allVehicles(after: String, first: Int, before: String, last: Int): VehiclesConnection
          vehicle(id: ID, vehicleID: ID): Vehicle
          """Fetches an object given its ID"""
          node("""The ID of an object""" id: ID!): Node
        }
        """A connection to a list of items."""
        type FilmsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [FilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        """Information about pagination in a connection."""
        type PageInfo {
          """When paginating forwards, are there more items?"""
          hasNextPage: Boolean!
          """When paginating backwards, are there more items?"""
          hasPreviousPage: Boolean!
          """When paginating backwards, the cursor to continue."""
          startCursor: String
          """When paginating forwards, the cursor to continue."""
          endCursor: String
        }
        """An edge in a connection."""
        type FilmsEdge {
          """The item at the end of the edge"""
          node: Film
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A single film."""
        type Film implements Node {
          """The title of this film."""
          title: String
          """The episode number of this film."""
          episodeID: Int
          """The opening paragraphs at the beginning of this film."""
          openingCrawl: String
          """The name of the director of this film."""
          director: String
          """The name(s) of the producer(s) of this film."""
          producers: [String]
          """The ISO 8601 date format of film release at original creator country."""
          releaseDate: String
          speciesConnection(after: String, first: Int, before: String, last: Int): FilmSpeciesConnection
          starshipConnection(after: String, first: Int, before: String, last: Int): FilmStarshipsConnection
          vehicleConnection(after: String, first: Int, before: String, last: Int): FilmVehiclesConnection
          characterConnection(after: String, first: Int, before: String, last: Int): FilmCharactersConnection
          planetConnection(after: String, first: Int, before: String, last: Int): FilmPlanetsConnection
          """The ISO 8601 date format of the time that this resource was created."""
          created: String
          """The ISO 8601 date format of the time that this resource was edited."""
          edited: String
          """The ID of an object"""
          id: ID!
        }
        """An object with an ID"""
        interface Node {
          """The id of the object."""
          id: ID!
        }
        """A connection to a list of items."""
        type FilmSpeciesConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [FilmSpeciesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          species: [Species]
        }
        """An edge in a connection."""
        type FilmSpeciesEdge {
          """The item at the end of the edge"""
          node: Species
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A type of person or character within the Star Wars Universe."""
        type Species implements Node {
          """The name of this species."""
          name: String
          """The classification of this species, such as "mammal" or "reptile"."""
          classification: String
          """The designation of this species, such as "sentient"."""
          designation: String
          """The average height of this species in centimeters."""
          averageHeight: Float
          """The average lifespan of this species in years, null if unknown."""
          averageLifespan: Int
          """
          Common eye colors for this species, null if this species does not typically
        have eyes.
          """
          eyeColors: [String]
          """
          Common hair colors for this species, null if this species does not typically
        have hair.
          """
          hairColors: [String]
          """
          Common skin colors for this species, null if this species does not typically
        have skin.
          """
          skinColors: [String]
          """The language commonly spoken by this species."""
          language: String
          """A planet that this species originates from."""
          homeworld: Planet
          personConnection(after: String, first: Int, before: String, last: Int): SpeciesPeopleConnection
          filmConnection(after: String, first: Int, before: String, last: Int): SpeciesFilmsConnection
          """The ISO 8601 date format of the time that this resource was created."""
          created: String
          """The ISO 8601 date format of the time that this resource was edited."""
          edited: String
          """The ID of an object"""
          id: ID!
        }
        """The `Float` scalar type represents signed double-precision fractional values as specified by [IEEE 754](https://en.wikipedia.org/wiki/IEEE_floating_point)."""
        scalar Float
        """
        A large mass, planet or planetoid in the Star Wars Universe, at the time of
        0 ABY.
        """
        type Planet implements Node {
          """The name of this planet."""
          name: String
          """The diameter of this planet in kilometers."""
          diameter: Int
          """
          The number of standard hours it takes for this planet to complete a single
        rotation on its axis.
          """
          rotationPeriod: Int
          """
          The number of standard days it takes for this planet to complete a single orbit
        of its local star.
          """
          orbitalPeriod: Int
          """
          A number denoting the gravity of this planet, where "1" is normal or 1 standard
        G. "2" is twice or 2 standard Gs. "0.5" is half or 0.5 standard Gs.
          """
          gravity: String
          """The average population of sentient beings inhabiting this planet."""
          population: Float
          """The climates of this planet."""
          climates: [String]
          """The terrains of this planet."""
          terrains: [String]
          """
          The percentage of the planet surface that is naturally occuring water or bodies
        of water.
          """
          surfaceWater: Float
          residentConnection(after: String, first: Int, before: String, last: Int): PlanetResidentsConnection
          filmConnection(after: String, first: Int, before: String, last: Int): PlanetFilmsConnection
          """The ISO 8601 date format of the time that this resource was created."""
          created: String
          """The ISO 8601 date format of the time that this resource was edited."""
          edited: String
          """The ID of an object"""
          id: ID!
        }
        """A connection to a list of items."""
        type PlanetResidentsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PlanetResidentsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          residents: [Person]
        }
        """An edge in a connection."""
        type PlanetResidentsEdge {
          """The item at the end of the edge"""
          node: Person
          """A cursor for use in pagination"""
          cursor: String!
        }
        """An individual person or character within the Star Wars universe."""
        type Person implements Node {
          """The name of this person."""
          name: String
          """
          The birth year of the person, using the in-universe standard of BBY or ABY -
        Before the Battle of Yavin or After the Battle of Yavin. The Battle of Yavin is
        a battle that occurs at the end of Star Wars episode IV: A New Hope.
          """
          birthYear: String
          """
          The eye color of this person. Will be "unknown" if not known or "n/a" if the
        person does not have an eye.
          """
          eyeColor: String
          """
          The gender of this person. Either "Male", "Female" or "unknown",
        "n/a" if the person does not have a gender.
          """
          gender: String
          """
          The hair color of this person. Will be "unknown" if not known or "n/a" if the
        person does not have hair.
          """
          hairColor: String
          """The height of the person in centimeters."""
          height: Int
          """The mass of the person in kilograms."""
          mass: Float
          """The skin color of this person."""
          skinColor: String
          """A planet that this person was born on or inhabits."""
          homeworld: Planet
          filmConnection(after: String, first: Int, before: String, last: Int): PersonFilmsConnection
          """The species that this person belongs to, or null if unknown."""
          species: Species
          starshipConnection(after: String, first: Int, before: String, last: Int): PersonStarshipsConnection
          vehicleConnection(after: String, first: Int, before: String, last: Int): PersonVehiclesConnection
          """The ISO 8601 date format of the time that this resource was created."""
          created: String
          """The ISO 8601 date format of the time that this resource was edited."""
          edited: String
          """The ID of an object"""
          id: ID!
        }
        """A connection to a list of items."""
        type PersonFilmsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PersonFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        """An edge in a connection."""
        type PersonFilmsEdge {
          """The item at the end of the edge"""
          node: Film
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type PersonStarshipsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PersonStarshipsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          starships: [Starship]
        }
        """An edge in a connection."""
        type PersonStarshipsEdge {
          """The item at the end of the edge"""
          node: Starship
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A single transport craft that has hyperdrive capability."""
        type Starship implements Node {
          """The name of this starship. The common name, such as "Death Star"."""
          name: String
          """
          The model or official name of this starship. Such as "T-65 X-wing" or "DS-1
        Orbital Battle Station".
          """
          model: String
          """
          The class of this starship, such as "Starfighter" or "Deep Space Mobile
        Battlestation"
          """
          starshipClass: String
          """The manufacturers of this starship."""
          manufacturers: [String]
          """The cost of this starship new, in galactic credits."""
          costInCredits: Float
          """The length of this starship in meters."""
          length: Float
          """The number of personnel needed to run or pilot this starship."""
          crew: String
          """The number of non-essential people this starship can transport."""
          passengers: String
          """
          The maximum speed of this starship in atmosphere. null if this starship is
        incapable of atmosphering flight.
          """
          maxAtmospheringSpeed: Int
          """The class of this starships hyperdrive."""
          hyperdriveRating: Float
          """
          The Maximum number of Megalights this starship can travel in a standard hour.
        A "Megalight" is a standard unit of distance and has never been defined before
        within the Star Wars universe. This figure is only really useful for measuring
        the difference in speed of starships. We can assume it is similar to AU, the
        distance between our Sun (Sol) and Earth.
          """
          MGLT: Int
          """The maximum number of kilograms that this starship can transport."""
          cargoCapacity: Float
          """
          The maximum length of time that this starship can provide consumables for its
        entire crew without having to resupply.
          """
          consumables: String
          pilotConnection(after: String, first: Int, before: String, last: Int): StarshipPilotsConnection
          filmConnection(after: String, first: Int, before: String, last: Int): StarshipFilmsConnection
          """The ISO 8601 date format of the time that this resource was created."""
          created: String
          """The ISO 8601 date format of the time that this resource was edited."""
          edited: String
          """The ID of an object"""
          id: ID!
        }
        """A connection to a list of items."""
        type StarshipPilotsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [StarshipPilotsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          pilots: [Person]
        }
        """An edge in a connection."""
        type StarshipPilotsEdge {
          """The item at the end of the edge"""
          node: Person
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type StarshipFilmsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [StarshipFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        """An edge in a connection."""
        type StarshipFilmsEdge {
          """The item at the end of the edge"""
          node: Film
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type PersonVehiclesConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PersonVehiclesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          vehicles: [Vehicle]
        }
        """An edge in a connection."""
        type PersonVehiclesEdge {
          """The item at the end of the edge"""
          node: Vehicle
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A single transport craft that does not have hyperdrive capability"""
        type Vehicle implements Node {
          """
          The name of this vehicle. The common name, such as "Sand Crawler" or "Speeder
        bike".
          """
          name: String
          """
          The model or official name of this vehicle. Such as "All-Terrain Attack
        Transport".
          """
          model: String
          """The class of this vehicle, such as "Wheeled" or "Repulsorcraft"."""
          vehicleClass: String
          """The manufacturers of this vehicle."""
          manufacturers: [String]
          """The cost of this vehicle new, in Galactic Credits."""
          costInCredits: Float
          """The length of this vehicle in meters."""
          length: Float
          """The number of personnel needed to run or pilot this vehicle."""
          crew: String
          """The number of non-essential people this vehicle can transport."""
          passengers: String
          """The maximum speed of this vehicle in atmosphere."""
          maxAtmospheringSpeed: Int
          """The maximum number of kilograms that this vehicle can transport."""
          cargoCapacity: Float
          """
          The maximum length of time that this vehicle can provide consumables for its
        entire crew without having to resupply.
          """
          consumables: String
          pilotConnection(after: String, first: Int, before: String, last: Int): VehiclePilotsConnection
          filmConnection(after: String, first: Int, before: String, last: Int): VehicleFilmsConnection
          """The ISO 8601 date format of the time that this resource was created."""
          created: String
          """The ISO 8601 date format of the time that this resource was edited."""
          edited: String
          """The ID of an object"""
          id: ID!
        }
        """A connection to a list of items."""
        type VehiclePilotsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [VehiclePilotsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          pilots: [Person]
        }
        """An edge in a connection."""
        type VehiclePilotsEdge {
          """The item at the end of the edge"""
          node: Person
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type VehicleFilmsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [VehicleFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        """An edge in a connection."""
        type VehicleFilmsEdge {
          """The item at the end of the edge"""
          node: Film
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type PlanetFilmsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PlanetFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        """An edge in a connection."""
        type PlanetFilmsEdge {
          """The item at the end of the edge"""
          node: Film
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type SpeciesPeopleConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [SpeciesPeopleEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          people: [Person]
        }
        """An edge in a connection."""
        type SpeciesPeopleEdge {
          """The item at the end of the edge"""
          node: Person
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type SpeciesFilmsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [SpeciesFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        """An edge in a connection."""
        type SpeciesFilmsEdge {
          """The item at the end of the edge"""
          node: Film
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type FilmStarshipsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [FilmStarshipsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          starships: [Starship]
        }
        """An edge in a connection."""
        type FilmStarshipsEdge {
          """The item at the end of the edge"""
          node: Starship
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type FilmVehiclesConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [FilmVehiclesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          vehicles: [Vehicle]
        }
        """An edge in a connection."""
        type FilmVehiclesEdge {
          """The item at the end of the edge"""
          node: Vehicle
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type FilmCharactersConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [FilmCharactersEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          characters: [Person]
        }
        """An edge in a connection."""
        type FilmCharactersEdge {
          """The item at the end of the edge"""
          node: Person
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type FilmPlanetsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [FilmPlanetsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          planets: [Planet]
        }
        """An edge in a connection."""
        type FilmPlanetsEdge {
          """The item at the end of the edge"""
          node: Planet
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type PeopleConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PeopleEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          people: [Person]
        }
        """An edge in a connection."""
        type PeopleEdge {
          """The item at the end of the edge"""
          node: Person
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type PlanetsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [PlanetsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          planets: [Planet]
        }
        """An edge in a connection."""
        type PlanetsEdge {
          """The item at the end of the edge"""
          node: Planet
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type SpeciesConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [SpeciesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          species: [Species]
        }
        """An edge in a connection."""
        type SpeciesEdge {
          """The item at the end of the edge"""
          node: Species
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type StarshipsConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [StarshipsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          starships: [Starship]
        }
        """An edge in a connection."""
        type StarshipsEdge {
          """The item at the end of the edge"""
          node: Starship
          """A cursor for use in pagination"""
          cursor: String!
        }
        """A connection to a list of items."""
        type VehiclesConnection {
          """Information to aid in pagination."""
          pageInfo: PageInfo!
          """A list of edges."""
          edges: [VehiclesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
        This allows a client to fetch the first five objects by passing "5" as the
        argument to "first", then fetch the total count so it could display "5 of 83",
        for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
        field provided for quickly exploring the API; rather than querying for
        "{ edges { node } }" when no edge data is needed, this field can be be used
        instead. Note that when clients like Relay need to fetch the "cursor" field on
        the edge to enable efficient pagination, this shortcut cannot be used, and the
        full "{ edges { node } }" version should be used instead.
          """
          vehicles: [Vehicle]
        }
        """An edge in a connection."""
        type VehiclesEdge {
          """The item at the end of the edge"""
          node: Vehicle
          """A cursor for use in pagination"""
          cursor: String!
        }
    "#}
        )
    }

    #[test]
    fn it_builds_schema_with_interfaces() {
        let file = File::open("tests/fixtures/interfaces.json").unwrap();
        let res: Response<IntrospectionResult> = serde_json::from_reader(file).unwrap();

        let data = res.data.unwrap();
        let schema = Schema::try_from(data).unwrap();
        assert_eq!(
            schema.encode(),
            indoc! { r#"
        type Query {
          """Fetch a simple list of products with an offset"""
          topProducts(first: Int = 5): [Product] @deprecated(reason: "Use `products` instead")
          """Fetch a paginated list of products based on a filter type."""
          products(first: Int = 5, after: Int = 0, type: ProductType): ProductConnection
          """
          The currently authenticated user root. All nodes off of this
        root will be authenticated as the current user
          """
          me: User
        }
        """The Product type represents all products within the system"""
        interface Product {
          """The primary identifier of products in the graph"""
          upc: String!
          """The display name of the product"""
          name: String
          """A simple integer price of the product in US dollars"""
          price: Int
          """How much the product weighs in kg"""
          weight: Int @deprecated(reason: "Not all product's have a weight")
          """A simple list of all reviews for a product"""
          reviews: [Review] @deprecated(reason: "The `reviews` field on product is deprecated to roll over the return
        type from a simple list to a paginated list. The easiest way to fix your
        operations is to alias the new field `reviewList` to `review`:
          
          {
            ... on Product {
              reviews: reviewList {
                edges {
                  review {
                    body
                  }
                }
              }
            }
          }
        
        Once all clients have updated, we will roll over this field and deprecate
        `reviewList` in favor of the field name `reviews` again")
          """
          A paginated list of reviews. This field naming is temporary while all clients
        migrate off of the un-paginated version of this field call reviews. To ease this migration,
        alias your usage of `reviewList` to `reviews` so that after the roll over is finished, you
        can remove the alias and use the final field name:
        
          {
            ... on Product {
              reviews: reviewList {
                edges {
                  review {
                    body
                  }
                }
              }
            }
          }
          """
          reviewList(first: Int = 5, after: Int = 0): ReviewConnection
        }
        """A review is any feedback about products across the graph"""
        type Review {
          id: ID!
          """The plain text version of the review"""
          body: String
          """The user who authored the review"""
          author: User
          """The product which this review is about"""
          product: Product
        }
        """The base User in Acephei"""
        type User {
          """A globally unique id for the user"""
          id: ID!
          """The users full name as provided"""
          name: String
          """The account username of the user"""
          username: String
          """A list of all reviews by the user"""
          reviews: [Review]
        }
        """A connection wrapper for lists of reviews"""
        type ReviewConnection {
          """Helpful metadata about the connection"""
          pageInfo: PageInfo
          """List of reviews returned by the search"""
          edges: [ReviewEdge]
        }
        """
        The PageInfo type provides pagination helpers for determining
        if more data can be fetched from the list
        """
        type PageInfo {
          """More items exist in the list"""
          hasNextPage: Boolean
          """Items earlier in the list exist"""
          hasPreviousPage: Boolean
        }
        """A connection edge for the Review type"""
        type ReviewEdge {
          review: Review
        }
        enum ProductType {
          LATEST
          TRENDING
        }
        """A connection wrapper for lists of products"""
        type ProductConnection {
          """Helpful metadata about the connection"""
          pageInfo: PageInfo
          """List of products returned by the search"""
          edges: [ProductEdge]
        }
        """A connection edge for the Product type"""
        type ProductEdge {
          product: Product
        }
        """The basic book in the graph"""
        type Book implements Product {
          """All books can be found by an isbn"""
          isbn: String!
          """The title of the book"""
          title: String
          """The year the book was published"""
          year: Int
          """A simple list of similar books"""
          similarBooks: [Book]
          reviews: [Review]
          reviewList(first: Int = 5, after: Int = 0): ReviewConnection
          """
          relatedReviews for a book use the knowledge of `similarBooks` from the books
        service to return related reviews that may be of interest to the user
          """
          relatedReviews(first: Int = 5, after: Int = 0): ReviewConnection
          """Since books are now products, we can also use their upc as a primary id"""
          upc: String!
          """The name of a book is the book's title + year published"""
          name(delimeter: String = " "): String
          price: Int
          weight: Int
        }
        """Information about the brand Amazon"""
        type Amazon {
          """The url of a referrer for a product"""
          referrer: String
        }
        """A union of all brands represented within the store"""
        union Brand = Ikea | Amazon
        """Information about the brand Ikea"""
        type Ikea {
          """Which asile to find an item"""
          asile: Int
        }
        """
        The Furniture type represents all products which are items
        of furniture.
        """
        type Furniture implements Product {
          """The modern primary identifier for furniture"""
          upc: String!
          """The SKU field is how furniture was previously stored, and still exists in some legacy systems"""
          sku: String!
          name: String
          price: Int
          """The brand of furniture"""
          brand: Brand
          weight: Int
          reviews: [Review]
          reviewList(first: Int = 5, after: Int = 0): ReviewConnection
        }
    "#}
        )
    }
}
