terraform {
  cloud {
    organization = "iinc"

    workspaces {
      name = "cloak"
    }
  }
}
