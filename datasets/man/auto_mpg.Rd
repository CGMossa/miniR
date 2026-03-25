\name{auto_mpg}
\alias{auto_mpg}
\title{Auto MPG Data}
\description{
A 398-row regression dataset about city-cycle fuel consumption. It is commonly
used in examples involving regression, missing values, and categorical coding.
}
\format{
A CSV file with 398 rows and 9 columns:
\describe{
  \item{mpg}{Miles per gallon.}
  \item{cylinders}{Number of cylinders.}
  \item{displacement}{Engine displacement.}
  \item{horsepower}{Horsepower; missing values are encoded as \code{?}.}
  \item{weight}{Vehicle weight.}
  \item{acceleration}{Acceleration.}
  \item{model_year}{Model year.}
  \item{origin}{Origin code.}
  \item{car_name}{Vehicle name.}
}
}
\details{
Local file: \file{datasets/data/auto_mpg.csv}. The repository stores a
normalized CSV copy with a header row added and the quoted car name preserved
from the UCI source file.
}
\source{
UCI Machine Learning Repository dataset page:
\url{https://archive.ics.uci.edu/dataset/9/auto}.
Original download:
\url{https://archive.ics.uci.edu/static/public/9/auto%2Bmpg.zip}.
License: CC BY 4.0.
}
\references{
Quinlan, R. (1993). Auto MPG [Dataset]. UCI Machine Learning Repository.
\url{https://doi.org/10.24432/C5859H}.
}
\keyword{datasets}
