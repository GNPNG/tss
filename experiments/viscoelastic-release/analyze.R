args <- commandArgs(trailingOnly=TRUE)
stopifnot(length(args) == 3)
data_dir <- args[1]
ref_dir <- args[2]
out_dir <- args[3]
stock <- read.table(file.path(data_dir, "stock.dat"), col.names=c("t","minor","major","D"))
regression <- read.table(file.path(ref_dir, "viscodrop.ref"))
external <- read.csv(file.path(ref_dir, "viscodrop.figueiredo"), header=FALSE)
stopifnot(nrow(stock)==101, all(stock$t==regression[[1]]), all(is.finite(as.matrix(stock))))
get_case <- function(name) {
    table <- read.csv(file.path(data_dir, name, "deformations.csv"), header=FALSE,
                      col.names=c("t","minor","major","D","area","polymer_rms"))
    stopifnot(nrow(table)==101, all(is.finite(as.matrix(table))),
              all(table$t==stock$t), all(table$area>0), all(table$D>=0), all(table$D<1))
    table
}
elastic <- get_case("oldroyd-b")
newton <- get_case("newtonian")
stopifnot(all(newton$polymer_rms==0), max(elastic$polymer_rms)>0)
stopifnot(elastic$D[1] == newton$D[1], elastic$area[1] == newton$area[1])
combined <- data.frame(shear_time=stock$t, continuous_D=stock$D,
                       oldroyd_b_D=elastic$D, newtonian_D=newton$D,
                       oldroyd_b_area=elastic$area, newtonian_area=newton$area,
                       oldroyd_b_polymer_stress_rms=elastic$polymer_rms)
write.csv(combined,file.path(out_dir,"deformation.csv"),row.names=FALSE)
errors <- approx(stock$t,stock$D,xout=external[[1]])$y-external[[2]]
stopifnot(all(is.finite(errors)))
report <- c(stock_vs_reference_max_abs=max(abs(stock$D-regression[[4]])),
            published_curve_samples=length(errors), published_curve_RMSE=sqrt(mean(errors^2)),
            max_rel_area_drift_elastic=max(abs(elastic$area/elastic$area[1]-1)),
            max_rel_area_drift_newtonian=max(abs(newton$area/newton$area[1]-1)))
print(report,digits=8)
print(combined[stock$t %in% c(0,2,2.4,4,6,10),],row.names=FALSE)

png(file.path(out_dir,"shear-release.png"), width=1100,height=760,res=118)
par(mfrow=c(2,2),mar=c(3.5,4.2,2.4,1),mgp=c(2.2,.65,0),las=1,bty="l")
purple <- "#6C418B"
green <- "#00736B"
plot(elastic$t,elastic$D,type="l",lwd=2.6,col=purple,ylim=c(0,.75),
     xlab="Shear time",ylab="Deformation D",main="Drop stretch and recovery")
lines(newton$t,newton$D,lwd=2,col=green,lty=2)
lines(stock$t,stock$D,col="grey48",lty=3,lwd=1.8)
abline(v=2,col="grey54",lty=3)
legend("bottomright",c("Oldroyd-B, stopped","Newtonian, stopped","Oldroyd-B, continuous"),
       col=c(purple,green,"grey48"),lty=1:3,lwd=2,bty="n",cex=.80)
plot(elastic$t,elastic$polymer_rms,type="l",col=purple,lwd=2.6,xlab="Shear time",
     ylab="Droplet-weighted RMS",main="Polymer extra stress")
abline(v=2,col="grey54",lty=3)
for(t in c(2,6)) {
    plot(0,0,type="n",asp=1,xlim=c(-2.1,2.1),ylim=c(-1.2,1.2),
         xlab="x / initial radius",ylab="y / initial radius",main=paste("Interface at shear time",t))
    for (case in c("oldroyd-b","newtonian")) {
        xy <- read.table(file.path(data_dir,case,sprintf("contour-%03d.dat",4*t)))
        stopifnot(nrow(xy) %% 2 == 0, ncol(xy)==2)
        i <- seq(1,nrow(xy),by=2)
        segments(xy[i,1],xy[i,2],xy[i+1,1],xy[i+1,2],col=if(case=="oldroyd-b") purple else green,
                 lwd=if(case=="oldroyd-b") 3 else 1)
    }
}
invisible(dev.off())
