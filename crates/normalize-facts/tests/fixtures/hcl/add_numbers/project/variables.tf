variable "base_number" {
  description = "The base number for arithmetic operations"
  type        = number
  default     = 5
}

variable "addend" {
  description = "Number to add to the base"
  type        = number
  default     = 3
}

variable "multiplier" {
  description = "Number to multiply by"
  type        = number
  default     = 2
}

variable "environment" {
  description = "Deployment environment name"
  type        = string
  default     = "dev"
}
