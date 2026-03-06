locals {
  sum     = var.base_number + var.addend
  product = var.base_number * var.multiplier
}

resource "null_resource" "compute" {
  triggers = {
    sum     = local.sum
    product = local.product
  }
}

output "sum" {
  description = "The sum of base_number and addend"
  value       = local.sum
}

output "product" {
  description = "The product of base_number and multiplier"
  value       = local.product
}
