\name{breast_cancer_wisconsin_diagnostic}
\alias{breast_cancer_wisconsin_diagnostic}
\title{Breast Cancer Wisconsin Diagnostic Data}
\description{
A 569-row binary classification dataset of cell-nuclei features computed from
fine needle aspirate images of breast masses.
}
\format{
A CSV file with 569 rows and 32 columns:
\describe{
  \item{id}{Identifier.}
  \item{diagnosis}{Target label: \code{M} for malignant or \code{B} for benign.}
  \item{radius_mean, ..., fractal_dimension_mean}{Ten mean feature measurements.}
  \item{radius_se, ..., fractal_dimension_se}{Ten standard-error feature measurements.}
  \item{radius_worst, ..., fractal_dimension_worst}{Ten worst-case feature measurements.}
}
}
\details{
Local file: \file{datasets/data/breast_cancer_wisconsin_diagnostic.csv}. The
repository stores a normalized CSV copy with a header row added to the UCI
source file.
}
\source{
UCI Machine Learning Repository dataset page:
\url{https://archive.ics.uci.edu/dataset/17/breast+cancer+wisconsin+diagnostic}.
Original download:
\url{https://archive.ics.uci.edu/static/public/17/breast%2Bcancer%2Bwisconsin%2Bdiagnostic.zip}.
License: CC BY 4.0.
}
\references{
Wolberg, W., Mangasarian, O., Street, N., and Street, W. (1993). Breast Cancer
Wisconsin (Diagnostic) [Dataset]. UCI Machine Learning Repository.
\url{https://doi.org/10.24432/C5DW2B}.
}
\keyword{datasets}
