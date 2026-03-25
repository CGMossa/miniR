\name{abalone}
\alias{abalone}
\title{Abalone Age Prediction Data}
\description{
A 4177-row dataset for predicting abalone age from physical measurements. It is
widely used in teaching examples for regression and classification.
}
\format{
A CSV file with 4177 rows and 9 columns:
\describe{
  \item{sex}{Sex code: M, F, or I.}
  \item{length_mm}{Longest shell measurement in millimeters.}
  \item{diameter_mm}{Diameter perpendicular to length in millimeters.}
  \item{height_mm}{Height with meat in shell in millimeters.}
  \item{whole_weight_g}{Whole abalone weight in grams.}
  \item{shucked_weight_g}{Weight of meat in grams.}
  \item{viscera_weight_g}{Gut weight after bleeding in grams.}
  \item{shell_weight_g}{Shell weight after drying in grams.}
  \item{rings}{Ring count; age is approximately \code{rings + 1.5}.}
}
}
\details{
Local file: \file{datasets/data/abalone.csv}. The repository stores a
normalized CSV copy with a header row added to the UCI source file.
}
\source{
UCI Machine Learning Repository dataset page:
\url{https://archive.ics.uci.edu/dataset/1/abalone}.
Original download:
\url{https://archive.ics.uci.edu/static/public/1/abalone.zip}.
License: CC BY 4.0.
}
\references{
Nash, W., Sellers, T., Talbot, S., Cawthorn, A., and Ford, W. (1994). Abalone
[Dataset]. UCI Machine Learning Repository.
\url{https://doi.org/10.24432/C55C7W}.
}
\keyword{datasets}
